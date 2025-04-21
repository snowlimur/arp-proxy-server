mod api;
mod cache;
mod config;
mod errors;
mod ingester;

use crate::cache::list_cache::ListCache;
use crate::cache::map_cache::MapCache;
use crate::cache::static_cache::ShardedStaticCache;
use crate::cache::Cache;
use crate::config::{CacheConfig, Setting};
use crate::errors::ServerError;
use crate::ingester::list_ingester::ListIngester;
use crate::ingester::map_ingester::MapIngester;
use crate::ingester::simple_ingester::SimpleIngester;
use crate::ingester::Ingester;
use api::http::server::{start_ingester, start_transmitter};
use bytes::Bytes;
use clap::Parser as ClapParser;
use std::fs;
use std::process;
use std::sync::Arc;
use tokio::sync::Notify;
use tokio::task::JoinSet;
use tracing::{error, info};
use tracing_subscriber::{fmt, layer::SubscriberExt};

#[derive(ClapParser, Debug)]
#[command(version)]
struct Cli {
    #[arg(short, long, default_value = "config.toml")]
    config: String,

    #[arg(short, long)]
    buffer: Option<usize>,

    cache: String,
}

fn main() {
    // Initialize tracing
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

    let args = Cli::parse();
    let setting = setting(&args);

    let runtime = common::runtime::build(setting.runtime.threads);
    if let Err(e) = runtime {
        error!("failed to create runtime: {}", e);
        process::exit(1);
    }

    let runtime = runtime.unwrap();
    let result = runtime.block_on(start(setting, &args.cache, args.buffer.clone()));
    if let Err(e) = result {
        error!("{}", e);
        process::exit(1);
    }

    info!("done");
}

fn setting(args: &Cli) -> Setting {
    let data = fs::read_to_string(args.config.as_str());
    if data.is_err() {
        error!("config file '{}' does not exist", args.config);
        process::exit(1);
    }

    let data = data.unwrap();
    let setting: Setting = toml::from_str(data.as_str()).unwrap();
    setting
}

async fn start(
    setting: Setting,
    cache_name: &str,
    buffer: Option<usize>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let cache_config = setting.cache.config(cache_name);
    let (cache, ingester) = match cache_config {
        CacheConfig::Static(config) => {
            info!("cache: {:?}", config);
            let file_content = tokio::fs::read(config.file_path)
                .await
                .map_err(|e| ServerError::StorageError(format!("Failed to read file: {}", e)))?;
            let data = Bytes::copy_from_slice(&file_content);
            let cache = Arc::new(ShardedStaticCache::new(
                config.shards,
                config.streams,
                config.tracks,
                config.segments,
                data,
            )) as Arc<dyn Cache + Send + Sync>;
            let ingester = Arc::new(SimpleIngester::new()) as Arc<dyn Ingester + Send + Sync>;
            (cache, ingester)
        }
        CacheConfig::List(config) => {
            info!("cache: {:?}", config);
            let cache = Arc::new(ListCache::new(config.copy));
            let ingester =
                Arc::new(ListIngester::new(Arc::clone(&cache))) as Arc<dyn Ingester + Send + Sync>;
            let cache = Arc::clone(&cache) as Arc<dyn Cache + Send + Sync>;
            (cache, ingester)
        }
        CacheConfig::Map(config) => {
            info!("cache: {:?}", config);
            let cache = Arc::new(MapCache::new(config.preallocate));
            let ingester =
                Arc::new(MapIngester::new(Arc::clone(&cache))) as Arc<dyn Ingester + Send + Sync>;
            let cache = Arc::clone(&cache) as Arc<dyn Cache + Send + Sync>;
            (cache, ingester)
        }
        _ => {
            panic!("Invalid cache config");
        }
    };

    let notifier = Arc::new(Notify::new());
    common::systemd::run(notifier.clone());

    let mut set = JoinSet::new();

    let addr = setting.ingester.addr;
    let max_buffer_size = buffer.clone();
    let notifier_clone = notifier.clone();
    set.spawn(async move {
        let ingester = Arc::clone(&ingester);
        let result = start_ingester(notifier_clone.clone(), addr, max_buffer_size, ingester).await;
        if let Err(e) = result {
            notifier_clone.notify_waiters();
            error!("ingester server: {}", e);
        }
    });

    let addr = setting.transmitter.addr;
    let max_buffer_size = buffer.clone();
    let notifier_clone = notifier.clone();
    set.spawn(async move {
        let cache = Arc::clone(&cache);
        let result = start_transmitter(notifier_clone.clone(), addr, max_buffer_size, cache).await;
        if let Err(e) = result {
            notifier_clone.notify_waiters();
            error!("transmitter server: {}", e);
        }
    });

    set.join_all().await;

    Ok(())
}
