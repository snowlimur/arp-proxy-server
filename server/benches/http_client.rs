use bytes::Bytes;
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use http_body_util::{BodyExt, Empty};
use hyper::body::Incoming;
use hyper::client::conn::http1;
use hyper::client::conn::http1::SendRequest;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use std::error::Error;
use std::fmt;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::TcpStream;
use tokio::runtime::Builder;
use tokio::sync::Mutex;
use tokio::task::JoinSet;
use tokio::time::sleep;

fn http_client(c: &mut Criterion) {
    bench(c, 1000, 5, 30);
}

fn bench(c: &mut Criterion, streams: u64, tracks: u64, segments: u64) {
    let core_count = num_cpus::get();
    let runtime = Builder::new_multi_thread()
        .worker_threads(core_count)
        .enable_all()
        .build()
        .expect("Не удалось создать Tokio Runtime");
    
    let mut group = c.benchmark_group("HTTP-Client");
    let addr = "127.0.0.1:8446";
    let host = "localhost";

    let concurrent_configs = [100, 500, 1000, 2000, 5000, 10000];
    for concurrent in concurrent_configs {
        group.throughput(Throughput::Elements(concurrent));

        group.bench_with_input(
            BenchmarkId::from_parameter(&concurrent),
            &concurrent,
            |b, &n_coroutines| {
                b.to_async(&runtime).iter_custom(|iters| async move {
                    let mut set = JoinSet::new();
                    let counter = Arc::new(AtomicU32::new(0));

                    let mut senders_pool = Vec::new();
                    for _ in 0..n_coroutines {
                        let sender = init_sender(addr, Arc::clone(&counter)).await;
                        if let Err(e) = sender {
                            panic!("{}", e);
                        }
                        senders_pool.push(Arc::new(Mutex::new(sender.unwrap())));
                    }

                    let start = Instant::now();

                    for i in 0..n_coroutines {
                        let sender_clone: Arc<Mutex<SendRequest<Empty<Bytes>>>> =
                            Arc::clone(&senders_pool[i as usize]);
                        set.spawn(async move {
                            let mut sender = sender_clone.lock().await;
                            for y in 0..iters {
                                let path = gen_key(y % streams, y % tracks, y % segments);
                                let res = request(&mut sender, host, &path).await;
                                if let Err(e) = res {
                                    panic!("request: {}", e);
                                }

                                let mut res = res.unwrap();
                                if res.status() != StatusCode::OK {
                                    panic!("{}: non OK status", path);
                                }

                                while let Some(next) = res.frame().await {
                                    if next.is_err() {
                                        if let Err(e) = next {
                                            panic!("response: {}", e);
                                        }
                                    }
                                }
                            }
                        });
                    }

                    set.join_all().await;
                    let elapsed = start.elapsed();

                    drop(senders_pool);
                    sleep(Duration::from_millis(100)).await;
                    let connections = counter.load(Relaxed);
                    if connections != 0 {
                        panic!("Not all connections are closed");
                    }

                    elapsed
                });
            },
        );
    }
    group.finish();
}

fn gen_key(stream: u64, track: u64, segment: u64) -> String {
    format!("/stream-{}/{}/{}.m4s", stream, track, segment)
}

async fn init_io(addr: &str) -> Result<TokioIo<TcpStream>, ClientError> {
    // let port = uri.port_u16().unwrap_or(80);
    // let addr = format!("{}:{}", uri.host().unwrap(), port);
    let result = TcpStream::connect(addr).await;
    if let Err(e) = result {
        return Err(ClientError::NetworkError(format!(
            "connect to {}: {}",
            addr, e
        )));
    }

    let tcp_stream = result.unwrap();
    Ok(TokioIo::new(tcp_stream))
}

async fn init_sender(
    addr: &str,
    counter: Arc<AtomicU32>,
) -> Result<SendRequest<Empty<Bytes>>, ClientError> {
    let io = init_io(addr).await?;

    let handshake = http1::handshake(io).await;
    if let Err(e) = handshake {
        return Err(ClientError::RequestError(format!("http1 handshake: {}", e)));
    }

    let (sender, conn) = handshake.unwrap();
    counter.fetch_add(1, Relaxed);
    tokio::task::spawn(async move {
        if let Err(err) = conn.await {
            println!("connection: {:?}", err);
        }
        counter.fetch_sub(1, Relaxed);
    });

    Ok(sender)
}

async fn request(
    sender: &mut SendRequest<Empty<Bytes>>,
    host: &str,
    path: &str,
) -> Result<Response<Incoming>, ClientError> {
    let req = Request::builder()
        .method(Method::GET)
        .uri(path)
        .header(hyper::header::HOST, host)
        .body(Empty::<Bytes>::new());

    if let Err(e) = req {
        return Err(ClientError::RequestError(format!("build request: {}", e)));
    }

    let req = req.unwrap();
    let res = sender.send_request(req).await;
    if let Err(e) = res {
        return Err(ClientError::RequestError(format!("send request: {}", e)));
    }

    Ok(res.unwrap())
}

#[derive(Debug)]
pub enum ClientError {
    ConfigError(String),
    NetworkError(String),
    StorageError(String),
    RequestError(String),
    InternalError(String),
}

impl fmt::Display for ClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClientError::ConfigError(msg) => write!(f, "configuration: {}", msg),
            ClientError::NetworkError(msg) => write!(f, "network: {}", msg),
            ClientError::StorageError(msg) => write!(f, "storage: {}", msg),
            ClientError::RequestError(msg) => write!(f, "request: {}", msg),
            ClientError::InternalError(msg) => write!(f, "internal: {}", msg),
        }
    }
}

impl Error for ClientError {}

criterion_group!(benches, http_client);
criterion_main!(benches);
