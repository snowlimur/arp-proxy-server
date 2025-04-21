mod config;
mod error;
mod service;
mod storage;
mod stream;

use crate::config::Settings;
use crate::error::RecorderError;
use crate::service::CMAFUploader;
use crate::storage::FileStorage;
use clap::Parser as ClapParser;
use hyper::server::conn::http1;
use hyper_util::rt::TokioIo;
use std::sync::Arc;
use std::{fs, process};
use tokio::net::TcpListener;
use tracing::{debug, error, info};

#[derive(ClapParser, Debug)]
#[command(version)]
struct Cli {
    #[arg(short, long, default_value = "recorder.toml")]
    config: String,
}

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt::init();

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

pub async fn run(settings: &Settings) -> Result<(), RecorderError> {
    let addr = common::socket::parse_address(settings.http.addr.clone())
        .map_err(|e| RecorderError::NetworkError(e.to_string()))?;
    let socket = common::socket::listen_reuse_socket(&addr)
        .map_err(|e| RecorderError::NetworkError(e.to_string()))?;
    let listener = TcpListener::from_std(socket.into())
        .map_err(|e| RecorderError::NetworkError(e.to_string()))?;

    info!("Listening on http://{}", addr);

    // Create the file storage system
    let storage = Arc::new(FileStorage::new(settings.storage.path.clone()));

    // Create the service with the storage
    let service = CMAFUploader::new(storage.clone(), settings.stream.inactive_timeout);
    let http = http1::Builder::new();

    loop {
        let (tcp_stream, remote_addr) = listener
            .accept()
            .await
            .map_err(|e| RecorderError::NetworkError(e.to_string()))?;

        let service = service.clone();
        let io = TokioIo::new(tcp_stream);
        let conn = http.serve_connection(io, service);

        debug!("Connection accepted from {}", remote_addr);
        tokio::spawn(async move {
            if let Err(e) = conn.await {
                error!("Connection error: {:?}", e);
            }
        });
    }
}

fn build_settings(config_path: &str) -> Result<Settings, RecorderError> {
    let data = fs::read_to_string(config_path).map_err(|_| {
        RecorderError::ConfigError(format!("Config file '{}' does not exist", config_path))
    })?;

    toml::from_str(&data)
        .map_err(|e| RecorderError::ConfigError(format!("Invalid configuration: {}", e)))
}
