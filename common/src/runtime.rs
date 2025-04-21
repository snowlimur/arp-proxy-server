use std::io;
use tokio::runtime::Runtime;
use tracing::info;

pub fn build(threads: Option<usize>) -> io::Result<Runtime> {
    let mut builder = tokio::runtime::Builder::new_multi_thread();
    builder.enable_all();
    if threads.is_some() {
        let threads = threads.unwrap();
        info!("custom runtime threads: {}", threads);
        builder.worker_threads(threads);
    }

    builder.build()
}
