use crate::api::http::service::{IngesterService, TransmitterService};
use crate::cache::Cache;
use crate::errors::ServerError;
use crate::ingester::Ingester;
use hyper::server::conn::http1;
use hyper_util::rt::TokioIo;
use std::pin;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::Notify;
use tracing::{error, info};

pub async fn start_ingester(
    notifier: Arc<Notify>,
    addr: String,
    max_buffer_size: Option<usize>,
    ingester: Arc<dyn Ingester + Send + Sync>,
) -> Result<(), ServerError> {
    let addr = common::socket::parse_address(addr.clone())
        .map_err(|e| ServerError::NetworkError(e.to_string()))?;
    let socket = common::socket::listen_reuse_socket(&addr)
        .map_err(|e| ServerError::NetworkError(e.to_string()))?;
    let listener = TcpListener::from_std(socket.into())
        .map_err(|e| ServerError::NetworkError(e.to_string()))?;

    info!("ingester: listening on http://{}", addr);

    let mut custom_buffer = false;
    let mut http = http1::Builder::new();
    if let Some(max_buffer_size) = max_buffer_size {
        if max_buffer_size > 0 {
            custom_buffer = true;
            info!("ingester: max buffer size is set to {}", max_buffer_size);
            http.max_buf_size(max_buffer_size);
        }
    }

    if !custom_buffer {
        info!("ingester: max buffer size is default ~400KB");
    }

    let graceful = hyper_util::server::graceful::GracefulShutdown::new();
    let mut signal = pin::pin!(notifier.notified());
    let ingester_service = IngesterService::new(Arc::clone(&ingester));

    loop {
        tokio::select! {
            Ok((stream, _addr)) = listener.accept() => {
                let service = ingester_service.clone();
                let io = TokioIo::new(stream);
                let conn = http.serve_connection(io, service);
                let fut = graceful.watch(conn);
                tokio::spawn(async move {
                    if let Err(e) = fut.await {
                        error!("ingester: downstream: serve: {:?}", e);
                    }
                });
            },
            _ = &mut signal => {
                info!("ingester: http server: graceful shutdown");
                break;
            }
        }
    }

    tokio::select! {
        _ = graceful.shutdown() => {
            info!("ingester: http server: all connections gracefully closed");
        },
        // @todo make it configurable
        _ = tokio::time::sleep(std::time::Duration::from_secs(30)) => {
            info!("ingester: timed out wait for all connections to close");
        }
    }
    Ok(())
}

pub async fn start_transmitter(
    notifier: Arc<Notify>,
    addr: String,
    max_buffer_size: Option<usize>,
    cache: Arc<dyn Cache + Send + Sync>,
) -> Result<(), ServerError> {
    let addr = common::socket::parse_address(addr.clone())
        .map_err(|e| ServerError::NetworkError(e.to_string()))?;
    let socket = common::socket::listen_reuse_socket(&addr)
        .map_err(|e| ServerError::NetworkError(e.to_string()))?;
    let listener = TcpListener::from_std(socket.into())
        .map_err(|e| ServerError::NetworkError(e.to_string()))?;

    info!("transmitter: listening on http://{}", addr);

    let mut custom_buffer = false;
    let mut http = http1::Builder::new();
    if let Some(max_buffer_size) = max_buffer_size {
        if max_buffer_size > 0 {
            custom_buffer = true;
            info!("transmitter: max buffer size is set to {}", max_buffer_size);
            http.max_buf_size(max_buffer_size);
        }
    }

    if !custom_buffer {
        info!("transmitter: max buffer size is default ~400KB");
    }

    let graceful = hyper_util::server::graceful::GracefulShutdown::new();
    let mut signal = pin::pin!(notifier.notified());
    let transmitter_service = Arc::new(TransmitterService::new(cache));

    loop {
        tokio::select! {
            Ok((stream, _addr)) = listener.accept() => {
                let service = Arc::clone(&transmitter_service);
                let io = TokioIo::new(stream);
                let conn = http.serve_connection(io, service);
                let fut = graceful.watch(conn);
                tokio::spawn(async move {
                    if let Err(e) = fut.await {
                        error!("transmitter: downstream: serve: {:?}", e);
                    }
                });
            },
            _ = &mut signal => {
                info!("transmitter: http server: graceful shutdown");
                break;
            }
        }
    }

    tokio::select! {
        _ = graceful.shutdown() => {
            info!("transmitter: http server: all connections gracefully closed");
        },
        // @todo make it configurable
        _ = tokio::time::sleep(std::time::Duration::from_secs(30)) => {
            info!("transmitter: timed out wait for all connections to close");
        }
    }
    Ok(())
}
