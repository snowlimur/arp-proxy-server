use bytes::Bytes;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use rustc_hash::{FxBuildHasher, FxHashMap};
use std::ptr;
use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

fn write(c: &mut Criterion) {
    let mut group = c.benchmark_group("LinkedList/write");
    group.throughput(Throughput::Elements(1));

    let data = Bytes::from_static(b"data");
    let list = LinkedList::new();

    let length_config = [5, 10, 20, 30, 40, 50, 60, 70, 80, 90, 100];
    for length in length_config {
        group.bench_with_input(
            BenchmarkId::new("Length", &length),
            &length,
            |b, &length| {
                b.iter_custom(|iters| {
                    let start = Instant::now();
                    for i in 0..iters {
                        list.insert(format!("{}.m4s", i), Some(data.clone()));
                        if list.len() >= length {
                            list.drop_tail();
                        }
                    }
                    start.elapsed()
                })
            },
        );
    }

    group.finish();
}

fn read(c: &mut Criterion) {
    let mut group = c.benchmark_group("Fx/LinkedList/read");
    group.throughput(Throughput::Elements(1));

    let data = Bytes::from_static(b"data");
    let list = LinkedList::new();
    let n = 100;
    for i in 0..n {
        list.insert(format!("{}.m4s", i), Some(data.clone()));
    }
    let list = Arc::new(list);

    let mut hm: FxHashMap<String, Arc<LinkedList>> =
        FxHashMap::with_hasher(FxBuildHasher::default());
    for i in 0..1_000_000 {
        hm.insert(format!("/stream/{}", i), Arc::clone(&list));
    }

    let stream_name = "/stream/0";
    let length_config = [1, 10, 20, 30, 40, 50, 60, 70, 80, 90, 100];
    for length in length_config {
        group.bench_with_input(
            BenchmarkId::new("Length", &length),
            &length,
            |b, &length| {
                let last_segment = format!("{}.m4s", length-1);
                b.iter_custom(|iters| {
                    let start = Instant::now();
                    for _ in 0..iters {
                        let l = hm.get(stream_name).unwrap();
                        if let Some(data) = l.get(&last_segment) {
                            black_box(data);
                        }
                    }
                    start.elapsed()
                })
            },
        );
    }

    group.finish();
}

criterion_group!(benches, read, write);
criterion_main!(benches);

pub struct LinkedList {
    count: AtomicUsize,
    tail: AtomicPtr<Node>,
    head: AtomicPtr<Node>,
}

pub struct Node {
    key: String,
    value: Option<Bytes>,
    next: AtomicPtr<Node>,
}

impl Node {
    pub fn new(key: String, value: Option<Bytes>) -> Node {
        Node {
            key,
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

impl LinkedList {
    pub fn new() -> Self {
        LinkedList {
            count: AtomicUsize::new(0),
            tail: AtomicPtr::new(ptr::null_mut()),
            head: AtomicPtr::new(ptr::null_mut()),
        }
    }

    pub fn len(&self) -> usize {
        self.count.load(Ordering::Relaxed)
    }

    pub fn tail(&self) -> Option<Arc<Node>> {
        strong_clone(self.tail.load(Ordering::Acquire))
    }

    pub fn insert(&self, key: String, value: Option<Bytes>) {
        let new_head = Arc::new(Node::new(key, value));
        let new_ptr = Arc::into_raw(new_head) as *mut Node;

        self.count.fetch_add(1, Ordering::Relaxed);
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

    pub fn get(&self, key: &str) -> Option<Bytes> {
        let cursor = self.tail();
        if cursor.is_none() {
            return None;
        }

        let mut cursor = cursor.unwrap();
        loop {
            if cursor.key == key {
                return cursor.value.clone();
            }

            let next = cursor.next();
            if next.is_none() {
                return None;
            }

            cursor = next.unwrap();
        }
    }

    pub fn drop_tail(&self) {
        let mut ptr = self.tail.load(Ordering::Acquire);
        if ptr.is_null() {
            return;
        }

        ptr = drop_node(ptr);
        self.tail.store(ptr, Ordering::Release);
        self.count.fetch_sub(1, Ordering::Relaxed);
    }

    fn drop_nodes(&self) {
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
        self.count.store(0, Ordering::Relaxed);
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
