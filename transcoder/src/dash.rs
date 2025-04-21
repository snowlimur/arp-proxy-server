use common::defaults::Bool;
use serde::Deserialize;
use std::process::Command;

#[derive(Deserialize, Debug)]
pub struct Dash {
    #[serde(default)]
    pub enabled: bool,
    /// Duration of each segment in seconds.
    pub segment_duration: f32,
    #[serde(default)]
    pub segment_type: SegmentType,
    /// Duration of each fragment within a segment in seconds.
    pub fragment_duration: f32,
    #[serde(default)]
    pub fragment_type: FragType,
    /// Target latency for live streaming in seconds.
    pub target_latency: f32,
    /// URL of the page that will return the UTC timestamp in ISO format.
    #[serde(default)]
    pub utc_timing_url: String,
    /// Write Producer Reference Time elements on supported streams. This also enables writing prft boxes in the underlying muxer.
    /// Applicable only when the utc_url option is enabled. It is set to auto by default,
    /// in which case the muxer will attempt to enable it only in modes that require it.
    #[serde(default)]
    pub write_prft: bool,

    /// Template for naming media segments.
    pub media_segment_name: String,
    /// Template for naming initialization segments.
    pub init_segment_name: String,

    #[serde(default)]
    pub format_options: String,
    #[serde(default)]
    pub adaptation_sets: String,
    #[serde(default)]
    pub select: String,

    /// Size of the sliding window for live streaming in seconds.
    pub window_size: u32,
    /// Set the maximum number of segments kept outside of the manifest before removing from disk.
    pub extra_window_size: u32,
    /// Enable or disable use of SegmentTemplate instead of SegmentList in the manifest. This is enabled by default.
    #[serde(default = "Bool::r#true")]
    pub use_template: bool,
    /// Enable or disable use of SegmentTimeline within the SegmentTemplate manifest section. This is enabled by default.
    #[serde(default = "Bool::r#true")]
    pub use_timeline: bool,
    /// Enable or disable segment index correction logic. Applicable only when use_template is enabled and use_timeline is disabled.
    /// This is disabled by default. When enabled, the logic monitors the flow of segment indexes.
    /// If a streams’s segment index value is not at the expected real time position, then the logic corrects that index value.
    /// Typically this logic is needed in live streaming use cases. The network bandwidth fluctuations are common during long run streaming.
    /// Each fluctuation can cause the segment indexes fall behind the expected real time position.
    #[serde(default)]
    pub index_correction: bool,
    /// Enables low latency streaming.
    #[serde(default)]
    pub low_latency: bool,
    /// Enables streaming mode for segment delivery.
    /// Affects how FFmpeg buffers and sends data to the server.
    #[serde(default)]
    pub streaming: bool,
    /// Ignore IO errors during open and write. Useful for long-duration runs with network output. This is disabled by default.
    #[serde(default)]
    pub ignore_io_errors: bool,
    /// Removes segments at exit.
    #[serde(default)]
    pub remove_at_exit: bool,
    /// Action to take on failure.
    #[serde(default)]
    pub onfail: String,
}

impl Dash {
    pub fn add_args(&self, cmd: &mut Command) {
        if !self.enabled {
            return;
        }

        cmd.arg("-f")
            .arg("dash")
            // setup segment & fragment durations
            .arg("-seg_duration")
            .arg(format!("{:.1}", self.segment_duration))
            .arg("-frag_duration")
            .arg(format!("{:.1}", self.fragment_duration))
            .arg("-frag_type")
            .arg(self.fragment_type.to_string())
            .arg("-target_latency")
            .arg(format!("{:.1}", self.target_latency))
            .arg("-ldash")
            .arg(format!("{}", self.low_latency as u8))
            .arg("-streaming")
            .arg(format!("{}", self.streaming as u8))
            .arg("-use_template")
            .arg(format!("{}", self.use_template as u8))
            .arg("-use_timeline")
            .arg(format!("{}", self.use_timeline as u8))
            .arg("-index_correction")
            .arg(format!("{}", self.index_correction as u8))
            .arg("-remove_at_exit")
            .arg(format!("{}", self.remove_at_exit as u8))
            .arg("-ignore_io_errors")
            .arg(format!("{}", self.ignore_io_errors as u8))
            .arg("-write_prft")
            .arg(format!("{}", self.write_prft as u8));

        if !self.media_segment_name.is_empty() {
            cmd.arg("-media_seg_name").arg(&self.media_segment_name);
        }
        if !self.init_segment_name.is_empty() {
            cmd.arg("-init_seg_name").arg(&self.init_segment_name);
        }
        if self.window_size > 0 {
            cmd.arg("-window_size").arg(format!("{}", self.window_size));
        }
        if self.extra_window_size > 0 {
            cmd.arg("-extra_window_size")
                .arg(format!("{}", self.extra_window_size));
        }
        if !self.utc_timing_url.is_empty() {
            cmd.arg("-utc_timing_url").arg(&self.utc_timing_url);
        }
        if !self.adaptation_sets.is_empty() {
            cmd.arg("-adaptation_sets").arg(&self.adaptation_sets);
        }
        // if !self.select.is_empty() {
        //     cmd.arg("-select").arg(&self.select);
        // }

        if !self.format_options.is_empty() {
            cmd.arg("-format_options").arg(&self.format_options);
        }

        if !self.onfail.is_empty() {
            cmd.arg("-onfail").arg(&self.onfail);
        }
    }
}

#[derive(Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FragType {
    /// Set one fragment per segment
    Auto,
    /// Fragment at every frame
    EveryFrame,
    /// Fragment at specific time intervals
    Duration,
    /// Fragment at keyframes and following P-Frame reordering (Video only, experimental)
    PFrames,
}

impl ToString for FragType {
    fn to_string(&self) -> String {
        match self {
            FragType::Auto => "auto",
            FragType::EveryFrame => "every_frame",
            FragType::Duration => "duration",
            FragType::PFrames => "pframes",
        }
        .to_string()
    }
}

impl Default for FragType {
    fn default() -> Self {
        FragType::Auto
    }
}

#[derive(Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SegmentType {
    /// The dash segment files format will be selected based on the stream codec. This is the default mode.
    Auto,
    /// The dash segment files will be in ISOBMFF/MP4 format
    MP4,
    /// The dash segment files will be in WebM format
    WebM,
}

impl ToString for SegmentType {
    fn to_string(&self) -> String {
        match self {
            SegmentType::Auto => "auto",
            SegmentType::MP4 => "mp4",
            SegmentType::WebM => "webm",
        }
        .to_string()
    }
}

impl Default for SegmentType {
    fn default() -> Self {
        SegmentType::Auto
    }
}
