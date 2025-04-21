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
use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll, Waker};
use tokio::sync::{Mutex, Notify};

#[derive(Debug, Clone)]
pub struct ListCache {
    pub copy_before_insert: bool,
    map: Arc<Mutex<HashMap<String, Arc<Cell>>>>,
}

impl ListCache {
    pub fn new(copy_before_insert: bool) -> Self {
        let map = Arc::new(Mutex::new(HashMap::new()));
        ListCache {
            map,
            copy_before_insert,
        }
    }

    pub async fn cell(&self, key: &str) -> Arc<Cell> {
        let mut locked_map = self.map.lock().await;
        let cell = locked_map.get(key);
        if let Some(cell) = cell {
            return Arc::clone(cell);
        }

        let cell = Arc::new(Cell::new());
        locked_map.insert(key.to_string(), Arc::clone(&cell));

        cell
    }

    pub async fn remove(&self, key: &str) {
        let mut locked_map = self.map.lock().await;
        locked_map.remove(key);
    }
}

#[async_trait]
impl Cache for ListCache {
    async fn get(&self, key: &str) -> Result<Option<BoxBody<Bytes, Infallible>>, ServerError> {
        let locked_map = self.map.lock().await;
        let data = locked_map.get(key);
        if data.is_none() {
            return Ok(None);
        }

        let data = data.unwrap();
        let downstream = ListDownstream::new(Arc::clone(&data));
        let body = StreamBody::new(downstream);
        Ok(Some(BoxBody::new(body)))
    }
}

#[derive(Debug, Clone)]
pub struct Cell {
    notifier: Arc<Notify>,
    data: Arc<LinkedList>,
}

impl Cell {
    pub fn new() -> Self {
        Cell {
            data: Arc::new(LinkedList::new()),
            notifier: Arc::new(Notify::new()),
        }
    }

    pub fn tail(&self) -> Option<Arc<Node>> {
        self.data.tail()
    }

    pub fn append(&self, data: Option<Bytes>) {
        self.data.insert(data);
        self.notifier.notify_waiters();
    }

    pub fn notifier(&self) -> Arc<Notify> {
        self.notifier.clone()
    }
}

impl Drop for Cell {
    fn drop(&mut self) {
        self.data.drop_nodes();
    }
}

struct ListDownstream {
    cell: Arc<Cell>,
    notifier: Arc<Notify>,
    cursor: Option<Arc<Node>>,
}

impl ListDownstream {
    pub fn new(data: Arc<Cell>) -> Self {
        let notifier = data.notifier();
        ListDownstream {
            notifier,
            cell: data,
            cursor: None,
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

impl Stream for ListDownstream {
    type Item = Result<Frame<Bytes>, Infallible>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let next: Option<Arc<Node>>;
        if let Some(node) = &self.cursor {
            next = node.next();
        } else {
            next = self.cell.tail();
        }

        if next.is_none() {
            return self.wait(cx.waker().clone());
        }

        let node = next.unwrap();
        self.cursor = Some(Arc::clone(&node)); // move cursor

        if let Some(data) = &node.value {
            let frame = Frame::data(data.clone());
            Poll::Ready(Some(Ok(frame)))
        } else {
            Poll::Ready(None)
        }
    }
}

#[derive(Debug)]
pub struct Node {
    value: Option<Bytes>,
    next: AtomicPtr<Node>,
}

impl Node {
    pub fn new(value: Option<Bytes>) -> Node {
        Node {
            value,
            next: AtomicPtr::new(ptr::null_mut()),
        }
    }

    pub fn next(&self) -> Option<Arc<Node>> {
        strong_clone(self.next.load(Ordering::Acquire))
    }

    fn set_next(&self, ptr: *mut Node) {
        self.next.store(ptr, Ordering::Release);
    }
}

#[derive(Debug)]
pub struct LinkedList {
    tail: AtomicPtr<Node>,
    head: AtomicPtr<Node>,
}

impl LinkedList {
    pub fn new() -> Self {
        LinkedList {
            tail: AtomicPtr::new(ptr::null_mut()),
            head: AtomicPtr::new(ptr::null_mut()),
        }
    }

    pub fn tail(&self) -> Option<Arc<Node>> {
        strong_clone(self.tail.load(Ordering::Acquire))
    }

    pub fn insert(&self, value: Option<Bytes>) {
        let new_head = Arc::new(Node::new(value));
        let new_ptr = Arc::into_raw(new_head) as *mut Node;

        if self.tail.load(Ordering::Acquire).is_null() {
            self.tail.store(copy_ptr(new_ptr), Ordering::Release)
        }

        let current_head = self.head.load(Ordering::Acquire);
        if !current_head.is_null() {
            let node = unsafe { Arc::from_raw(current_head) };
            node.set_next(copy_ptr(new_ptr));
        }

        self.head.store(new_ptr, Ordering::Release);
    }

    pub fn drop_nodes(&self) {
        let mut ptr = self.tail.load(Ordering::Acquire);
        if ptr.is_null() {
            return;
        }

        loop {
            ptr = drop_node(ptr);
            if ptr.is_null() {
                break;
            }
        }

        self.tail.store(ptr, Ordering::Release);
    }
}

impl Drop for LinkedList {
    fn drop(&mut self) {
        self.drop_nodes();
        drop_node(self.head.load(Ordering::Acquire));
        drop_node(self.tail.load(Ordering::Acquire));
    }
}

fn strong_clone(ptr: *mut Node) -> Option<Arc<Node>> {
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

fn drop_node(ptr: *mut Node) -> *mut Node {
    if ptr.is_null() {
        return ptr;
    }

    let node = unsafe { Arc::from_raw(ptr) };
    node.next.load(Ordering::Acquire)
}

fn copy_ptr(ptr: *mut Node) -> *mut Node {
    if ptr.is_null() {
        return ptr;
    }

    unsafe {
        let origin = Arc::from_raw(ptr);
        let clone = Arc::clone(&origin);
        let _ = Arc::into_raw(origin);

        Arc::into_raw(clone) as *mut Node
    }
}
