use crate::ingester::Ingester;
use async_trait::async_trait;
use http_body_util::BodyExt;
use hyper::body::Incoming;
use hyper::{Method, Request};
use tracing::error;

#[derive(Debug, Clone)]
pub struct SimpleIngester;

impl SimpleIngester {
    pub fn new() -> Self {
        SimpleIngester {}
    }
}

#[async_trait]
impl Ingester for SimpleIngester {
    async fn ingest(&self, mut req: Request<Incoming>) {
        if req.method() != Method::PUT {
            return;
        }

        while let Some(next) = req.frame().await {
            if next.is_err() {
                if let Err(e) = next {
                    error!("req body: read: {:?}", e);
                    break;
                }
            }

            let frame = next.unwrap();
            if frame.is_data() {
                let _ = frame.into_data().unwrap();
            }
        }
    }
}
