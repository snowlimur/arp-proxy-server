use crate::cache::map_cache::MapCache;
use crate::ingester::Ingester;
use async_trait::async_trait;
use bytes::{Buf, BufMut, BytesMut};
use http_body_util::BodyExt;
use hyper::body::Incoming;
use hyper::{Method, Request};
use std::sync::Arc;
use tracing::error;

#[derive(Debug, Clone)]
pub struct MapIngester {
    cache: Arc<MapCache>,
}

impl MapIngester {
    pub fn new(cache: Arc<MapCache>) -> Self {
        MapIngester { cache }
    }
}

#[async_trait]
impl Ingester for MapIngester {
    async fn ingest(&self, mut req: Request<Incoming>) {
        if req.method() != Method::PUT {
            return;
        }

        let key = req.uri().path().to_string();
        let mut buffer: BytesMut = if self.cache.preallocate > 0 {
            BytesMut::with_capacity(self.cache.preallocate)
        } else {
            BytesMut::new()
        };

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
                buffer.put(data);
                let data = Arc::new(buffer.copy_to_bytes(buffer.len()));
                self.cache.insert(key.as_str(), data, false).await;
            }
        }
        self.cache
            .insert(key.as_str(), Arc::new(buffer.freeze()), true)
            .await;
        self.cache.remove(key.as_str()).await;
    }
}
