use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::runtime::Builder;
use tokio::task::JoinSet;
use uuid::{Uuid};
use papaya::HashMap;

fn hashmap(c: &mut Criterion) {
    let runtime = Builder::new_multi_thread().enable_all().build().unwrap();

    let mut group = c.benchmark_group("Papaya");
    let concurrent_configs = [
        1, 100, 1000, 2000, 3000, 5000, 6000, 7000, 8000, 9000, 10000,
    ];
    let segments = 5;
    let tracks = 5;

    for concurrent in concurrent_configs {
        group.throughput(Throughput::Elements(concurrent as u64));
        group.bench_with_input(
            BenchmarkId::new("Concurrent", &concurrent),
            &concurrent,
            |b, &concurrent| {
                b.to_async(&runtime).iter_custom(|iters| async move {
                    let (stream_names, map) = make_map(concurrent as u64, tracks, segments);
                    let mut set = JoinSet::new();
                    let start = Instant::now();

                    for i in 0..concurrent {
                        let map_clone = Arc::clone(&map);
                        let stream_names = Arc::clone(&stream_names);
                        set.spawn(async move {
                            for y in 0..iters {
                                {
                                    let key = if y == 0 {
                                        format!("/{}/0/init.m4s", &stream_names[i])
                                    } else {
                                        format!("/{}/0/{}.m4s", &stream_names[i], y)
                                    };

                                    map_clone.pin().get_or_insert_with(key, || y);
                                }
                            }
                        });
                    }

                    set.join_all().await;
                    start.elapsed()
                });
            },
        );
    }
    group.finish();
}

fn make_map(
    streams: u64,
    tracks: u64,
    segments: u64,
) -> (
    Arc<Vec<String>>,
    Arc<HashMap<String, u64>>,
) {
    let mut stream_names = Vec::with_capacity(streams as usize);
    for _ in 0..streams {
        stream_names.push(Uuid::new_v4().to_string());
    }
    
    let map = HashMap::new();
    for stream in &stream_names {
        for t in 0..tracks {
            for s in 0..segments {
                let key = if s == 0 {
                    format!("{}/{}/init.m4s", stream, t)
                } else {
                    format!("{}/{}/{}.m4s", stream, t, s)
                };
                map.pin().insert(key, s);
            }
        }
    }

    (Arc::new(stream_names), Arc::new(map))
}

criterion_group!(benches, hashmap);
criterion_main!(benches);
