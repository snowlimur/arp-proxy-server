use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use dashmap::DashMap;
use fnv::{FnvBuildHasher, FnvHashMap};
use rustc_hash::{FxBuildHasher, FxHashMap};
use std::collections::HashMap;
use std::time::Instant;
use uuid::Uuid;
use papaya::HashMap as PapayaHashMap;

fn hashmap(c: &mut Criterion) {
    let n = 1_000_000u64;
    let mut keys = Vec::with_capacity(n as usize);
    for _ in 0..n {
        keys.push(Uuid::new_v4().to_string());
    }

    let mut hm: HashMap<String, u64> = HashMap::with_capacity(keys.len());
    for i in 0..n {
        let key = keys[i as usize].clone();
        hm.insert(key, i);
    }

    let mut group = c.benchmark_group("Unlocked");
    group.throughput(Throughput::Elements(1));
    group.bench_function("std", |b| {
        b.iter_custom(|iters| {
            let mut _sum: u64 = 0;
            let start = Instant::now();
            for i in 0..iters {
                let i = (i as usize) % keys.len();
                let key = &keys[i];
                if let Some(x) = hm.get(key) {
                    _sum += *x;
                }
            }
            start.elapsed()
        })
    });

    let mut hm: FnvHashMap<String, u64> =
        FnvHashMap::with_capacity_and_hasher(n as usize, FnvBuildHasher::default());
    for i in 0..n {
        let key = keys[i as usize].clone();
        hm.insert(key, i);
    }

    group.bench_function("fnv", |b| {
        b.iter_custom(|iters| {
            let mut _sum: u64 = 0;
            let start = Instant::now();
            for i in 0..iters {
                let i = (i as usize) % keys.len();
                let key = &keys[i];
                if let Some(x) = hm.get(key) {
                    _sum += *x;
                }
            }
            start.elapsed()
        })
    });

    let mut hm: FxHashMap<String, u64> =
        FxHashMap::with_capacity_and_hasher(n as usize, FxBuildHasher::default());
    for i in 0..n {
        let key = keys[i as usize].clone();
        hm.insert(key, i);
    }

    group.bench_function("rustc", |b| {
        b.iter_custom(|iters| {
            let mut _sum: u64 = 0;
            let start = Instant::now();
            for i in 0..iters {
                let i = (i as usize) % keys.len();
                let key = &keys[i];
                if let Some(x) = hm.get(key) {
                    _sum += *x;
                }
            }
            start.elapsed()
        })
    });

    let hm: DashMap<String, u64> = DashMap::with_capacity(n as usize);
    for i in 0..n {
        let key = keys[i as usize].clone();
        hm.insert(key, i);
    }

    group.bench_function("dashmap", |b| {
        b.iter_custom(|iters| {
            let mut _sum: u64 = 0;
            let start = Instant::now();
            for i in 0..iters {
                let i = (i as usize) % keys.len();
                let key = &keys[i];
                if let Some(x) = hm.get(key) {
                    _sum += *x;
                }
            }
            start.elapsed()
        })
    });

    let hm: PapayaHashMap<String, u64> = PapayaHashMap::with_capacity(n as usize);
    let hm = hm.pin();
    for i in 0..n {
        let key = keys[i as usize].clone();
        hm.insert(key, i);
    }

    group.bench_function("papaya", |b| {
        b.iter_custom(|iters| {
            let mut _sum: u64 = 0;
            let start = Instant::now();
            for i in 0..iters {
                let i = (i as usize) % keys.len();
                let key = &keys[i];
                if let Some(x) = hm.get(key) {
                    _sum += *x;
                }
            }
            start.elapsed()
        })
    });
    
    let hm: flurry::HashMap<String, u64> = flurry::HashMap::with_capacity(n as usize);
    let hm = hm.pin();
    for i in 0..n {
        let key = keys[i as usize].clone();
        hm.insert(key, i);
    }

    group.bench_function("flurry", |b| {
        b.iter_custom(|iters| {
            let mut _sum: u64 = 0;
            let start = Instant::now();
            for i in 0..iters {
                let i = (i as usize) % keys.len();
                let key = &keys[i];
                if let Some(x) = hm.get(key) {
                    _sum += *x;
                }
            }
            start.elapsed()
        })
    });

    group.finish();
}

criterion_group!(benches, hashmap);
criterion_main!(benches);
