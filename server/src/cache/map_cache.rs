use crate::cache::Cache;
use crate::errors::ServerError;
use async_trait::async_trait;
use bytes::Bytes;
use futures_util::Stream;
use http_body_util::combinators::BoxBody;
use http_body_util::StreamBody;
use hyper::body::Frame;
use std::collections::HashMap;
use std::convert::Infallible;
use std::fmt::Debug;
use std::pin::Pin;
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll, Waker};
use tokio::sync::{Mutex, Notify};
use tracing::error;

#[derive(Debug, Clone)]
pub struct MapCache {
    pub preallocate: usize,
    map: Arc<Mutex<HashMap<String, Arc<Cell>>>>,
}

impl MapCache {
    pub fn new(preallocate: usize) -> Self {
        let map = Arc::new(Mutex::new(HashMap::new()));
        MapCache { map, preallocate }
    }

    pub async fn insert(&self, key: &str, data: Arc<Bytes>, completed: bool) {
        let mut locked_map = self.map.lock().await;
        let cell = locked_map.get(key);
        if let Some(cell) = cell {
            cell.set_data(data, completed);
            return;
        }

        let cell = Cell::new();
        cell.set_data(data, completed);
        locked_map.insert(key.to_string(), Arc::new(cell));
    }

    pub async fn remove(&self, key: &str) {
        let mut locked_map = self.map.lock().await;
        locked_map.remove(key);
    }
}

#[async_trait]
impl Cache for MapCache {
    async fn get(&self, key: &str) -> Result<Option<BoxBody<Bytes, Infallible>>, ServerError> {
        let locked_map = self.map.lock().await;
        let data = locked_map.get(key);
        if data.is_none() {
            return Ok(None);
        }

        let data = data.unwrap();
        let downstream = CellDownstream::new(Arc::clone(&data));
        let body = StreamBody::new(downstream);
        Ok(Some(BoxBody::new(body)))
    }
}

#[derive(Debug, Clone)]
struct Cell {
    completed: Arc<AtomicBool>,
    notifier: Arc<Notify>,
    data: Arc<AtomicPtr<Bytes>>,
}

impl Cell {
    pub fn new() -> Self {
        Cell {
            completed: Arc::new(AtomicBool::new(false)),
            data: Arc::new(AtomicPtr::new(ptr::null_mut())),
            notifier: Arc::new(Notify::new()),
        }
    }

    pub fn data(&self) -> Option<Arc<Bytes>> {
        let ptr = self.data.load(Ordering::Relaxed);
        if ptr.is_null() {
            return None;
        }

        let node = unsafe {
            let head = Arc::from_raw(ptr);
            let node = Arc::clone(&head);
            let _ = Arc::into_raw(head); // avoid dropping the variable
            node
        };

        Some(node)
    }

    pub fn set_data(&self, data: Arc<Bytes>, completed: bool) {
        self.drop_data();

        let data = Arc::clone(&data);
        let ptr = Arc::into_raw(data) as *mut Bytes;
        self.data.store(ptr, Ordering::Relaxed);
        self.completed.store(completed, Ordering::Relaxed);
        self.notifier.notify_waiters();
    }

    fn drop_data(&self) {
        let ptr = self.data.load(Ordering::Relaxed);
        if ptr.is_null() {
            return;
        }

        let node = unsafe { Arc::from_raw(ptr) };
        drop(node);
    }

    pub fn completed(&self) -> bool {
        self.completed.load(Ordering::Relaxed)
    }

    pub fn notifier(&self) -> Arc<Notify> {
        self.notifier.clone()
    }
}

impl Drop for Cell {
    fn drop(&mut self) {
        self.drop_data();
    }
}

struct CellDownstream {
    bytes_sent: usize,
    cell: Arc<Cell>,
    notifier: Arc<Notify>,
}

impl CellDownstream {
    pub fn new(data: Arc<Cell>) -> Self {
        let notifier = data.notifier();
        CellDownstream {
            cell: data,
            notifier,
            bytes_sent: 0,
        }
    }

    fn wait(&self, waker: Waker) -> Poll<Option<Result<Frame<Bytes>, Infallible>>> {
        let notifier = Arc::clone(&self.notifier);
        tokio::task::spawn(async move {
            notifier.notified().await;
            waker.wake();
        });

        Poll::Pending
    }
}

impl Stream for CellDownstream {
    type Item = Result<Frame<Bytes>, Infallible>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let cell_completed = self.cell.completed();
        let data = self.cell.data();
        if data.is_none() && cell_completed {
            return Poll::Ready(None);
        }

        let data = data.unwrap();
        let buffer_size = data.len();
        if buffer_size < self.bytes_sent {
            error!("Invalid data");
            return Poll::Ready(None);
        }

        if buffer_size == self.bytes_sent {
            if cell_completed {
                return Poll::Ready(None);
            }

            return self.wait(cx.waker().clone());
        }

        let chunk = data.slice(self.bytes_sent..buffer_size);
        self.bytes_sent = buffer_size;

        let frame = Frame::data(chunk);
        Poll::Ready(Some(Ok(frame)))
    }
}
