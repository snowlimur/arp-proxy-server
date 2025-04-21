use serde::Deserialize;
use std::process::Command;

use super::dash::Dash;

#[derive(Deserialize, Debug)]
pub struct Transcoder {
    /// General encoding settings that apply to all quality profiles.
    /// Contains parameters like preset, tune, and keyframe settings.
    pub encoding: Encoding,
    /// DASH-specific configuration parameters.
    /// Defines how the video stream should be segmented and packaged.
    pub dash: Dash,
    /// HTTP server configuration for segment delivery.
    /// Specifies how the transcoded segments should be distributed.
    pub http: Http,
    /// List of quality profiles for adaptive streaming.
    /// Each profile represents a different resolution/bitrate combination.
    pub videos: Vec<VideoProfile>,
    pub audio: Option<AudioProfile>,
}

impl Transcoder {
    /// Builds a complete FFmpeg command from the configuration.
    /// This method combines all the configuration aspects into a single FFmpeg command line.
    pub fn build_ffmpeg_command(
        &self,
        input: String,
        mut output: String,
    ) -> Result<Command, String> {
        let mut cmd = Command::new("ffmpeg");

        if self.encoding.native_rate {
            cmd.arg("-re");
        }

        cmd.arg("-i").arg(&input);

        // Build the filter complex string
        let filter_complex = self.build_filter_complex()?;
        cmd.arg("-filter_complex").arg(&filter_complex);

        self.add_profiles_encoding(&mut cmd)?;
        self.dash.add_args(&mut cmd);
        self.add_http_config(&mut cmd)?;

        if !self.http.base_url.is_empty() {
            output = format!("{}/{}", self.http.base_url.trim_end_matches('/'), output);
        }

        cmd.arg(output);

        Ok(cmd)
    }

    /// Builds the filter complex string that defines the video processing pipeline.
    /// This creates the necessary splits and scaling operations for each quality variant.
    fn build_filter_complex(&self) -> Result<String, String> {
        let mut filters = Vec::new();

        // First, split the input into as many streams as we have profiles
        let split = format!(
            "[v:0]split={}{}",
            self.videos.len(),
            self.videos
                .iter()
                .enumerate()
                .map(|(i, _)| format!("[v{}]", i))
                .collect::<Vec<_>>()
                .join("")
        );
        filters.push(split);

        // For each profile, add necessary processing filters
        for (i, profile) in self.videos.iter().enumerate() {
            let mut profile_filters = Vec::new();

            if profile.setsar != "" {
                profile_filters.push(format!("setsar={}", profile.setsar));
            }

            if profile.scale != "" {
                profile_filters.push(format!("scale={}", profile.scale));
            }

            if profile_filters.is_empty() {
                // If no processing is needed, just rename the stream
                filters.push(format!("[v{}]copy[{}_out]", i, profile.name));
            } else {
                filters.push(format!(
                    "[v{}]{}[{}_out]",
                    i,
                    profile_filters.join(","),
                    profile.name
                ));
            }
        }

        Ok(filters.join(";"))
    }

    /// Adds encoding parameters for each quality profile to the command.
    fn add_profiles_encoding(&self, cmd: &mut Command) -> Result<(), String> {
        for (_, video) in self.videos.iter().enumerate() {
            cmd.arg("-map").arg(format!("[{}_out]", video.name));
        }
        if let Some(_) = &self.audio {
            cmd.arg("-map").arg("a:0");
        }

        for (i, video) in self.videos.iter().enumerate() {
            self.encoding.add_args(cmd, i);
            video.add_args(cmd, i);
        }
        if let Some(audio) = &self.audio {
            audio.add_args(cmd, 0);
        }

        Ok(())
    }

    fn add_http_config(&self, cmd: &mut Command) -> Result<(), String> {
        if !self.http.method.is_empty() {
            cmd.arg("-method").arg(self.http.method.to_string());
        }
        if self.http.persistent {
            cmd.arg("-http_persistent").arg("1");
        }

        Ok(())
    }
}

/// Encoding defines the common encoding parameters that will be applied
/// to all video streams. These settings affect both the encoding speed and
/// the quality of the output video.
#[derive(Deserialize, Debug)]
pub struct Encoding {
    /// FFmpeg preset (e.g., "veryfast", "medium", "slow").
    /// Controls the speed/compression ratio trade-off.
    /// Faster presets result in larger file sizes at the same quality.
    pub preset: Preset,
    /// FFmpeg tune parameter (e.g., "zerolatency", "film", "animation").
    /// Optimizes encoding settings for specific types of content.
    pub tune: Tune,
    /// GOP (Group of Pictures) size - defines the interval between keyframes.
    /// Lower values provide better seeking precision but reduce compression efficiency.
    pub keyframe_interval: u32,
    /// Scene cut threshold for adaptive GOP placement.
    /// Controls how aggressively the encoder should insert keyframes at scene changes.
    /// Value of 0 disables adaptive keyframe placement.
    pub sc_threshold: u32,
    /// Video stream codec (e.g., "h264").
    pub video_codec: String,
    /// Audio stream codec (e.g., "aac").
    pub audio_codec: String,
    /// Indicates whether the input should be read at its native frame rate.
    /// When set to true, FFmpeg will process the input at the frame rate it was recorded.
    /// This can be useful for live streaming scenarios where maintaining the original
    /// timing of the input is important.
    #[serde(default)]
    pub native_rate: bool,
}

impl Encoding {
    fn add_args(&self, cmd: &mut Command, idx: usize) {
        cmd.arg(format!("-c:v:{}", idx))
            .arg(self.video_codec.to_string())
            .arg(format!("-g:v:{}", idx))
            .arg(self.keyframe_interval.to_string());

        if self.preset != Preset::None {
            cmd.arg(format!("-preset:v:{}", idx))
                .arg(self.preset.to_string());
        }
        if self.tune != Tune::None {
            cmd.arg(format!("-tune:v:{}", idx))
                .arg(self.tune.to_string());
        }

        cmd.arg(format!("-c:a:{}", idx))
            .arg(self.audio_codec.to_string())
            .arg("-sc_threshold")
            .arg(self.sc_threshold.to_string());
    }
}

/// Http defines how the transcoded segments should be delivered
/// to the HTTP server. These settings affect the networking behavior
/// of the FFmpeg process.
#[derive(Deserialize, Debug)]
pub struct Http {
    /// Base URL where segments will be uploaded.
    /// Should point to an HTTP server that accepts PUT/POST requests.
    pub base_url: String,
    /// Whether to keep HTTP connections alive between segment uploads.
    /// Enabling this reduces connection overhead but requires server support.
    pub persistent: bool,
    /// HTTP method to use for segment upload (e.g., "PUT", "POST").
    /// Should match the server's expected request method.
    pub method: String,
}

#[derive(Deserialize, Debug)]
pub struct VideoProfile {
    /// Human-readable name for this quality profile (e.g., "1080p", "720p").
    /// Used in playlist generation and segment naming.
    pub name: String,
    /// Scaling factor or resolution for the video stream.
    /// Determines the output dimensions of the video.
    pub scale: String,
    /// Sample aspect ratio for the video stream.
    /// Adjusts the aspect ratio of the video frames.
    pub setsar: String,
    /// Target video bitrate (e.g., "6000k").
    /// Average bitrate that the encoder will try to maintain.
    pub bitrate: String,
    /// Maximum allowed video bitrate (e.g., "6600k").
    /// Helps control quality spikes in complex scenes.
    #[serde(default)]
    pub maxrate: String,
    /// Video buffer size (e.g., "12000k").
    /// Affects how strictly the encoder adheres to the target bitrate.
    #[serde(default)]
    pub bufsize: String,
    /// Video profile level.
    /// Determines the set of features and constraints for the video stream.
    #[serde(default)]
    pub profile: VideoProfileLevel,
}

impl VideoProfile {
    fn add_args(&self, cmd: &mut Command, idx: usize) {
        if self.profile != VideoProfileLevel::None {
            cmd.arg(format!("-profile:v:{}", idx))
                .arg(self.profile.to_string());
        }
        if !self.bitrate.is_empty() {
            cmd.arg(format!("-b:v:{}", idx)).arg(&self.bitrate);
        }
        if !self.maxrate.is_empty() {
            cmd.arg(format!("-maxrate:v:{}", idx)).arg(&self.maxrate);
        }
        if !self.bufsize.is_empty() {
            cmd.arg(format!("-bufsize:v:{}", idx)).arg(&self.bufsize);
        }
    }
}

#[derive(Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
/// Enum representing different levels of video profile.
/// Each level defines a set of features and constraints for video encoding.
pub enum VideoProfileLevel {
    None,
    /// Baseline profile, suitable for low-complexity applications.
    Baseline,
    /// Main profile, offering a balance between complexity and quality.
    Main,
    /// High profile, providing higher quality at the cost of increased complexity.
    High,
    /// High 10 profile, supporting 10-bit color depth.
    High10,
    /// High 4:2:2 profile, supporting 4:2:2 chroma subsampling.
    High422,
    /// High 4:4:4 profile, supporting 4:4:4 chroma subsampling.
    High444,
}

impl Default for VideoProfileLevel {
    fn default() -> Self {
        VideoProfileLevel::None
    }
}

impl ToString for VideoProfileLevel {
    fn to_string(&self) -> String {
        match self {
            VideoProfileLevel::None => "none",
            VideoProfileLevel::Baseline => "baseline",
            VideoProfileLevel::Main => "main",
            VideoProfileLevel::High => "high",
            VideoProfileLevel::High10 => "high10",
            VideoProfileLevel::High422 => "high422",
            VideoProfileLevel::High444 => "high444",
        }
        .to_string()
    }
}

#[derive(Deserialize, Debug)]
pub struct AudioProfile {
    /// Audio stream bitrate (e.g., "192k").
    /// Higher values improve audio quality but increase bandwidth usage.
    pub bitrate: String,
    /// Audio sampling rate in Hz (e.g., 48000).
    /// Common values are 44100 and 48000 Hz.
    pub sampling: u32,
    /// Number of audio channels (e.g., 2 for stereo).
    pub channels: u16,
}

impl AudioProfile {
    fn add_args(&self, cmd: &mut Command, idx: usize) {
        cmd.arg(format!("-ar:a:{}", idx))
            .arg(self.sampling.to_string())
            .arg(format!("-b:a:{}", idx))
            .arg(&self.bitrate);
    }
}

/// FFmpeg presets control the encoding speed and compression efficiency trade-off.
/// Faster presets result in larger file sizes at the same quality level.
#[derive(Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Preset {
    None,
    UltraFast,
    SuperFast,
    VeryFast,
    Faster,
    Fast,
    Medium,
    Slow,
    Slower,
    VerySlow,
    Placebo,
}

impl Default for Preset {
    fn default() -> Self {
        Preset::None
    }
}

impl ToString for Preset {
    fn to_string(&self) -> String {
        match self {
            Preset::None => "none",
            Preset::UltraFast => "ultrafast",
            Preset::SuperFast => "superfast",
            Preset::VeryFast => "veryfast",
            Preset::Faster => "faster",
            Preset::Fast => "fast",
            Preset::Medium => "medium",
            Preset::Slow => "slow",
            Preset::Slower => "slower",
            Preset::VerySlow => "veryslow",
            Preset::Placebo => "placebo",
        }
        .to_string()
    }
}

/// FFmpeg tune parameters optimize encoding for specific types of content.
/// Each option adjusts various encoding parameters to better handle certain characteristics.
#[derive(Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Tune {
    /// Optimize for fast encoding and low latency streaming
    ZeroLatency,
    /// Optimize for film content (live-action)
    Film,
    /// Optimize for animation content
    Animation,
    /// Optimize for grain retention in grainy content
    Grain,
    /// Optimize for screen recording
    Screen,
    /// Default tuning, balanced for general content
    None,
}

impl Default for Tune {
    fn default() -> Self {
        Tune::None
    }
}

impl ToString for Tune {
    fn to_string(&self) -> String {
        match self {
            Tune::ZeroLatency => "zerolatency",
            Tune::Film => "film",
            Tune::Animation => "animation",
            Tune::Grain => "grain",
            Tune::Screen => "screen",
            Tune::None => "none",
        }
        .to_string()
    }
}

/// HLS segment types define the container format for segments.
/// Different formats offer various features and compatibility options.
#[derive(Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
pub enum SegmentType {
    /// MPEG-2 Transport Stream segments
    Mpegts,
    /// Fragmented MP4 segments
    Fmp4,
}

impl ToString for SegmentType {
    fn to_string(&self) -> String {
        match self {
            SegmentType::Mpegts => "mpegts",
            SegmentType::Fmp4 => "fmp4",
        }
        .to_string()
    }
}

/// HLS playlist types define the behavior of the playlist updates.
#[derive(Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
pub enum PlaylistType {
    /// VOD (Video on Demand) - playlist remains static
    Vod,
    /// Event - segments can be added but not removed
    Event,
    /// No type - segments can be added and removed (live-streaming)
    None,
}

impl ToString for PlaylistType {
    fn to_string(&self) -> String {
        match self {
            PlaylistType::Vod => "vod",
            PlaylistType::Event => "event",
            PlaylistType::None => "none",
        }
        .to_string()
    }
}
