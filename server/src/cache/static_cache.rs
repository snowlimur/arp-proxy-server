use crate::cache::Cache;
use crate::errors::ServerError;
use async_trait::async_trait;
use bytes::Bytes;
use futures_util::Stream;
use http_body_util::combinators::BoxBody;
use http_body_util::StreamBody;
use hyper::body::Frame;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::convert::Infallible;
use std::hash::{Hash, Hasher};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

#[derive(Debug, Clone)]
pub struct StaticCache {
    map: Arc<HashMap<String, Bytes>>,
}

impl StaticCache {
    #[allow(dead_code)]
    pub fn new(streams: u64, tracks: u64, segments: u64, data: Bytes) -> Self {
        let map = make_map(streams, tracks, segments, data);
        StaticCache { map }
    }
}

#[async_trait]
impl Cache for StaticCache {
    async fn get(&self, key: &str) -> Result<Option<BoxBody<Bytes, Infallible>>, ServerError> {
        let data = self.map.get(key);
        if data.is_none() {
            return Ok(None);
        }

        let data = data.unwrap();
        let downstream = StaticDownstream::from(data.clone());
        let body = StreamBody::new(downstream);
        Ok(Some(BoxBody::new(body)))
    }
}

#[derive(Debug, Clone)]
pub struct ShardedStaticCache {
    shards: u64,
    map: Arc<HashMap<u64, Mutex<HashMap<String, Bytes>>>>,
}

impl ShardedStaticCache {
    pub fn new(shards: u64, streams: u64, tracks: u64, segments: u64, data: Bytes) -> Self {
        let map = make_mutex_map(shards, streams, tracks, segments, data);
        ShardedStaticCache { shards, map }
    }
}

#[async_trait]
impl Cache for ShardedStaticCache {
    async fn get(&self, key: &str) -> Result<Option<BoxBody<Bytes, Infallible>>, ServerError> {
        let i = shard(key, self.shards);
        let locked_map = self.map.get(&i).unwrap().lock().unwrap();
        let data = locked_map.get(key);
        if data.is_none() {
            return Ok(None);
        }

        let data = data.unwrap();
        let downstream = StaticDownstream::from(data.clone());
        let body = StreamBody::new(downstream);
        Ok(Some(BoxBody::new(body)))
    }
}

fn make_mutex_map(
    shards: u64,
    streams: u64,
    tracks: u64,
    segments: u64,
    data: Bytes,
) -> Arc<HashMap<u64, Mutex<HashMap<String, Bytes>>>> {
    let mut map = HashMap::new();
    for i in 0..shards {
        let shard: HashMap<String, Bytes> = HashMap::new();
        map.insert(i, Mutex::new(shard));
    }

    for x in 0..streams {
        for y in 0..tracks {
            for z in 0..segments {
                let key = gen_key(x, y, z);
                let shard = shard(&key, shards);
                map.get(&shard)
                    .unwrap()
                    .lock()
                    .unwrap()
                    .insert(key, data.clone());
            }
        }
    }

    Arc::new(map)
}

struct StaticDownstream {
    sent: bool,
    data: Bytes,
}

impl StaticDownstream {
    pub fn from(data: Bytes) -> Self {
        StaticDownstream { data, sent: false }
    }
}

impl Stream for StaticDownstream {
    type Item = Result<Frame<Bytes>, Infallible>;

    fn poll_next(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.sent {
            return Poll::Ready(None);
        }

        self.sent = true;
        let frame = Frame::data(self.data.clone());
        Poll::Ready(Some(Ok(frame)))
    }
}

#[allow(dead_code)]
fn make_map(streams: u64, tracks: u64, segments: u64, data: Bytes) -> Arc<HashMap<String, Bytes>> {
    let mut map = HashMap::new();
    for x in 1..streams {
        for y in 0..tracks {
            for z in 1..segments {
                let key = gen_key(x, y, z);
                map.insert(key, data.clone());
            }
        }
    }

    Arc::new(map)
}

fn gen_key(stream: u64, track: u64, segment: u64) -> String {
    format!("/stream-{}/{}/{}.m4s", stream, track, segment)
}

fn shard(input: &str, shards: u64) -> u64 {
    let mut hasher = DefaultHasher::new();
    input.hash(&mut hasher);

    let hash_value = hasher.finish();
    hash_value % shards
}
