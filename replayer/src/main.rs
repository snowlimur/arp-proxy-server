mod config;
mod error;
mod replayer;
mod storage;
mod stream;

use crate::config::Settings;
use crate::error::ReplayerError;
use crate::storage::FileStorage;
use crate::stream::StreamMetadata;
use clap::Parser as ClapParser;
use hyper::Uri;
use replayer::Replayer;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use std::{fs, process};
use tokio::task::JoinSet;
use tokio::time::Instant;
use tracing::{error, info};
use tracing_subscriber::{fmt, layer::SubscriberExt};

#[derive(ClapParser, Debug)]
#[command(version)]
struct Cli {
    #[arg(short, long, default_value = "config.toml")]
    config: String,
}

#[tokio::main]
async fn main() {
    let subscriber = tracing_subscriber::registry().with(
        fmt::Layer::default()
            .with_target(false)
            .with_thread_names(false)
            .with_ansi(true)
            .with_line_number(false)
            .with_file(false)
            .with_thread_ids(false),
    );
    tracing::subscriber::set_global_default(subscriber)
        .expect("Unable to set a global logger instance");

    let cli = Cli::parse();
    let settings = match build_settings(cli.config.as_str()) {
        Ok(settings) => settings,
        Err(e) => {
            error!("Failed to load configuration: {}", e);
            process::exit(1);
        }
    };

    if let Err(e) = run(&settings).await {
        error!("Server error: {}", e);
        process::exit(1);
    }
}

pub async fn run(settings: &Settings) -> Result<(), ReplayerError> {
    let uri = Uri::try_from(settings.target.url.as_str()).map_err(|e| {
        ReplayerError::ConfigError(format!("Invalid target URL: {}", e.to_string()))
    })?;

    // Create the file storage system
    let mut storage = FileStorage::new(settings.storage.path.clone());
    for step in &settings.schedule.steps {
        storage.read_metadata(&step.stream).await?;
    }

    let storage = Arc::new(storage);
    for step in &settings.schedule.steps {
        let start = Instant::now();
        info!("Running step: {} / {}", step.stream, step.parallel);

        if let Some(delay) = step.delay {
            info!("▶ Sleeping for {}s", delay.as_secs());
            tokio::time::sleep(delay).await;
        }

        if let Some(duration) = step.duration {
            info!("▶ Duration: {}s", duration.as_secs());
        } else {
            info!("▶ All segments will be played once");
        }

        let metadata = match storage.get_metadata(&step.stream) {
            Some(meta) => meta,
            None => {
                return Err(ReplayerError::StorageError("No metadata found".to_string()));
            }
        };

        let bytes_sent = Arc::new(AtomicUsize::new(0));
        let mut set = JoinSet::new();
        for i in 0..step.parallel {
            let uri = uri.clone();
            let metadata = metadata.clone();
            let storage = storage.clone();
            let duration = step.duration.clone();
            let bytes_sent = bytes_sent.clone();
            set.spawn(async move {
                let res = play(i, metadata, storage, uri, duration).await;
                if res.is_err() {
                    error!("thread {}: {}", i, res.err().unwrap());
                    return;
                }

                bytes_sent.fetch_add(res.unwrap(), Ordering::Relaxed);
            });
        }
        set.join_all().await;

        let elapsed = start.elapsed();
        let mb = bytes_sent.load(Ordering::Relaxed) / 1024 / 1024 * 8;
        let mbps = mb as f64 / elapsed.as_secs_f64();
        info!(
            "▶ Step is finished: {} s. Bytes sent: {} Mb. Speed: {:.2} Mbps",
            elapsed.as_secs(),
            mb,
            mbps
        );
    }

    Ok(())
}

async fn play(
    i: u32,
    meta: Arc<StreamMetadata>,
    storage: Arc<FileStorage>,
    uri: Uri,
    duration: Option<Duration>,
) -> Result<usize, ReplayerError> {
    let bytes_sent = Arc::new(AtomicUsize::new(0));
    let mut set = JoinSet::new();
    for representation in meta.representations.iter() {
        let replayer = Replayer::new(representation.clone(), storage.clone());
        let uri = uri.clone();
        let pnq = if let Some(path_and_query) = uri.path_and_query() {
            let path = format!("{}test-{}", path_and_query.path(), i);
            let query = path_and_query.query().unwrap_or("");
            if query.is_empty() {
                path
            } else {
                format!("{}?{}", path, query)
            }
        } else {
            format!("/test-{}", i)
        };

        let uri = Uri::builder()
            .scheme(uri.scheme_str().unwrap_or("http"))
            .authority(uri.authority().unwrap().to_string())
            .path_and_query(pnq)
            .build()
            .unwrap();

        let bytes_sent = bytes_sent.clone();
        set.spawn(async move {
            let res = replayer.play(uri.clone(), duration).await;
            if res.is_err() {
                error!("replay: uri {}: {}", uri.to_string(), res.err().unwrap());
                return;
            }

            bytes_sent.fetch_add(res.unwrap(), Ordering::Relaxed);
        });
    }

    set.join_all().await;
    Ok(bytes_sent.load(Ordering::Relaxed))
}

fn build_settings(config_path: &str) -> Result<Settings, ReplayerError> {
    let data = fs::read_to_string(config_path).map_err(|_| {
        ReplayerError::ConfigError(format!("Config file '{}' does not exist", config_path))
    })?;

    toml::from_str(&data)
        .map_err(|e| ReplayerError::ConfigError(format!("Invalid configuration: {}", e)))
}
