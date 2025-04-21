use crate::error::ReplayerError;
use crate::storage::FileStorage;
use crate::stream::{FileMetadata, RepresentationMetadata};
use bytes::Bytes;
use http_body_util::combinators::BoxBody;
use http_body_util::StreamBody;
use hyper::body::Frame;
use hyper::client::conn::http1;
use hyper::{header, Method, Request, Uri};
use hyper_util::rt::TokioIo;
use std::convert::Infallible;
use std::pin::Pin;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::time::Instant;
use tokio_stream::Stream;
use tracing::error;

pub struct Replayer {
    meta: Arc<RepresentationMetadata>,
    storage: Arc<FileStorage>,
}

impl Replayer {
    pub fn new(meta: Arc<RepresentationMetadata>, storage: Arc<FileStorage>) -> Self {
        Self { meta, storage }
    }

    pub async fn play(&self, uri: Uri, duration: Option<Duration>) -> Result<usize, ReplayerError> {
        let (mut sender, conn) = self.connect(&uri).await?;
        tokio::task::spawn(async move {
            if let Err(err) = conn.await {
                error!("connection: {:?}", err);
            }
        });

        let start = Instant::now();
        let mut total_bytes_sent: usize = 0;
        let mut last_time_offset: u32 = 0;

        if self.meta.init.is_some() {
            let init = self.meta.init.clone().unwrap();
            let uri = Self::build_init_uri(uri.clone(), self.meta.idx);
            let bytes_sent = self.send_file(&mut sender, uri, init.clone()).await?;
            total_bytes_sent += bytes_sent;
            last_time_offset = init.time_offset + init.chunks[init.chunks.len() - 1].0
        }

        let mut i = 0;
        let max = self.meta.segments.len();
        if max == 0 {
            return Err(ReplayerError::StorageError("no segments".to_string()));
        }

        loop {
            if duration.is_some() {
                if start.elapsed().gt(duration.as_ref().unwrap()) {
                    break;
                }
            }

            if i >= max {
                if duration.is_none() {
                    break;
                }

                i = 0;
                last_time_offset = 0;
            }

            let segment = self.meta.segments[i].clone();
            let delay = segment.time_offset - last_time_offset;
            if delay > 2 {
                tokio::time::sleep(Duration::from_millis(delay as u64)).await;
            }

            let uri = Self::build_segment_uri(uri.clone(), self.meta.idx, segment.segment.unwrap());
            let bytes_sent = self.send_file(&mut sender, uri, segment.clone()).await?;
            total_bytes_sent += bytes_sent;

            i = i + 1;
            last_time_offset = segment.time_offset + segment.chunks[segment.chunks.len() - 1].0
        }

        Ok(total_bytes_sent)
    }

    fn build_init_uri(uri: Uri, representation: u32) -> Uri {
        let pnq = uri.path_and_query();
        let path = if pnq.is_some() {
            let pnq = pnq.unwrap();
            let path = format!("{}/{}/init.m4s", pnq.path(), representation);
            let query = pnq.query().unwrap_or("");
            if query.is_empty() {
                path
            } else {
                format!("{}?{}", path, query)
            }
        } else {
            format!("/{}/init.m4s", representation)
        };

        Uri::builder()
            .scheme(uri.scheme_str().unwrap_or("http"))
            .authority(uri.authority().unwrap().to_string())
            .path_and_query(path)
            .build()
            .unwrap()
    }

    fn build_segment_uri(uri: Uri, representation: u32, segment: u32) -> Uri {
        let pnq = uri.path_and_query();
        let path = if pnq.is_some() {
            let pnq = pnq.unwrap();
            let path = format!(
                "{}/{}/{}.m4s",
                pnq.path(),
                representation,
                segment.to_string()
            );
            let query = pnq.query().unwrap_or("");
            if query.is_empty() {
                path
            } else {
                format!("{}?{}", path, query)
            }
        } else {
            format!("/{}/{}", representation, segment.to_string())
        };

        Uri::builder()
            .scheme(uri.scheme_str().unwrap_or("http"))
            .authority(uri.authority().unwrap().to_string())
            .path_and_query(path)
            .build()
            .unwrap()
    }

    async fn send_file(
        &self,
        sender: &mut http1::SendRequest<BoxBody<Bytes, Infallible>>,
        uri: Uri,
        file: Arc<FileMetadata>,
    ) -> Result<usize, ReplayerError> {
        let data = self.storage.get_file(file.file_name.as_str());
        if data.is_none() {
            return Err(ReplayerError::StorageError(format!(
                "file not found: {}",
                file.file_name
            )));
        }

        let stream = FileStream::from(file.clone(), data.unwrap());
        let bytes_sent = stream.bytes_sent();
        let body = StreamBody::new(stream);
        let body = BoxBody::new(body);

        let req = Request::builder()
            .method(Method::PUT)
            .uri(uri)
            .header(header::USER_AGENT, "dash-replayer/1.0")
            .header(header::TRANSFER_ENCODING, "chunked") // Important for streaming
            .body(body);

        if let Err(e) = req {
            return Err(ReplayerError::RequestError(format!("build request: {}", e)));
        }

        let req = req.unwrap();
        let res = sender.send_request(req).await;
        if let Err(e) = res {
            return Err(ReplayerError::RequestError(format!("send request: {}", e)));
        }

        Ok(bytes_sent.load(Relaxed))
    }

    async fn connect(
        &self,
        uri: &Uri,
    ) -> Result<
        (
            http1::SendRequest<BoxBody<Bytes, Infallible>>,
            http1::Connection<TokioIo<TcpStream>, BoxBody<Bytes, Infallible>>,
        ),
        ReplayerError,
    > {
        let io = self.get_io(uri).await?;
        let handshake = http1::handshake(io).await;
        if let Err(e) = handshake {
            return Err(ReplayerError::RequestError(format!(
                "http1 handshake: {}",
                e
            )));
        }

        let (sender, conn) = handshake.unwrap();
        Ok((sender, conn))
    }

    async fn get_io(&self, uri: &Uri) -> Result<TokioIo<TcpStream>, ReplayerError> {
        let port = uri.port_u16().unwrap_or(80);
        let addr = format!("{}:{}", uri.host().unwrap(), port);
        let tcp_stream = TcpStream::connect(addr.as_str()).await;
        if let Err(e) = tcp_stream {
            return Err(ReplayerError::NetworkError(format!(
                "connect to {}: {}",
                addr, e
            )));
        }

        Ok(TokioIo::new(tcp_stream.unwrap()))
    }
}

struct FileStream {
    file: Arc<FileMetadata>,
    last_idx: Option<usize>,
    bytes_sent: Arc<AtomicUsize>,
    data: Bytes,
    start: Instant,
    // The time offset of the last chunk sent
    wait: Option<Duration>,
    // The next wake time for the stream
    next_wake: Option<Duration>,
}

impl FileStream {
    pub fn from(file: Arc<FileMetadata>, data: Bytes) -> Self {
        FileStream {
            start: Instant::now(),
            bytes_sent: Arc::new(AtomicUsize::new(0)),
            last_idx: None,
            wait: None,
            next_wake: None,
            file,
            data,
        }
    }
}

impl FileStream {
    pub fn bytes_sent(&self) -> Arc<AtomicUsize> {
        self.bytes_sent.clone()
    }
}

impl Stream for FileStream {
    type Item = Result<Frame<Bytes>, Infallible>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let elapsed = self.start.elapsed();
        if self.next_wake.is_some() {
            let wake = self.next_wake.as_ref().unwrap();
            if elapsed < *wake {
                return Poll::Pending;
            }
            self.next_wake = None;
        }

        if let Some(delay) = self.wait {
            // Reset the wait flag as we've started the waiting process
            self.wait = None;
            self.next_wake = Some(elapsed + delay);
            wait(cx, delay);
            return Poll::Pending;
        }

        let idx = if let Some(idx) = self.last_idx {
            idx + 1
        } else {
            0
        };

        let chunks = &self.file.chunks;
        let max_idx = chunks.len();
        if idx >= max_idx {
            return Poll::Ready(None);
        }

        let (ts, offset, size) = chunks[idx];
        let next_idx = idx + 1;
        if next_idx < max_idx {
            let (next_ts, _, _) = chunks[next_idx];
            let delay = next_ts - ts;
            if delay > 2 {
                self.wait = Some(Duration::from_millis(delay as u64));
            }
        }

        self.last_idx = Some(idx);
        let frame = Frame::data(self.data.slice(offset..offset + size));
        self.bytes_sent().fetch_add(size, Relaxed);
        Poll::Ready(Some(Ok(frame)))
    }
}

fn wait(cx: &mut Context<'_>, delay: Duration) {
    let waker = cx.waker().clone();

    // Schedule a timer and register the waker to be notified when the timer completes
    tokio::spawn(async move {
        tokio::time::sleep(delay).await;
        waker.wake();
    });
}
