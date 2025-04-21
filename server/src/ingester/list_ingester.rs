use crate::cache::list_cache::ListCache;
use crate::ingester::Ingester;
use async_trait::async_trait;
use bytes::Bytes;
use http_body_util::BodyExt;
use hyper::body::Incoming;
use hyper::{Method, Request};
use std::sync::Arc;
use tracing::error;

#[derive(Debug, Clone)]
pub struct ListIngester {
    cache: Arc<ListCache>,
}

impl ListIngester {
    pub fn new(cache: Arc<ListCache>) -> Self {
        ListIngester { cache }
    }
}

#[async_trait]
impl Ingester for ListIngester {
    async fn ingest(&self, mut req: Request<Incoming>) {
        if req.method() != Method::PUT {
            return;
        }

        let key = req.uri().path().to_string();
        let cell = self.cache.cell(&key).await;
        while let Some(next) = req.frame().await {
            if next.is_err() {
                if let Err(e) = next {
                    error!("req body: read: {:?}", e);
                    break;
                }
            }

            let frame = next.unwrap();
            if frame.is_data() {
                let data = frame.into_data().unwrap();
                let data = if self.cache.copy_before_insert {
                    Bytes::copy_from_slice(&data)
                } else {
                    data
                };

                cell.append(Some(data));
            }
        }
        cell.append(None); // close cell
        self.cache.remove(key.as_str()).await;
    }
}
