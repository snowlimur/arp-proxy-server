use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use fnv::{FnvHashMap, FnvHasher};
use tokio::runtime::Builder;
use tokio::task::JoinSet;
use uuid::{Uuid};

fn hashmap(c: &mut Criterion) {
    // let core_count = num_cpus::get();
    // let threads_configs = [1, core_count / 2, core_count];
    let concurrent_configs = [
        1, 100, 1000, 2000, 3000, 5000, 6000, 7000, 8000, 9000, 10000,
    ];

    for concurrent in concurrent_configs {
        one_reader_one_stream(c, concurrent);
    }
}

fn one_reader_one_stream(c: &mut Criterion, concurrent: usize) {
    let runtime = Builder::new_multi_thread().enable_all().build().unwrap();

    let group_name = format!("Fnv-{}", concurrent);
    let mut group = c.benchmark_group(group_name);
    let shards_configs = [1, 2, 50, 100, 500, 1000, 5000, 10000];
    let segments = 5;
    let tracks = 5;

    for shards in shards_configs {
        group.throughput(Throughput::Elements(concurrent as u64));
        group.bench_with_input(
            BenchmarkId::new("Shards", &shards),
            &shards,
            |b, &shards| {
                b.to_async(&runtime).iter_custom(|iters| async move {
                    let (stream_names, map) = make_map(shards, concurrent as u64, tracks, segments);
                    let mut set = JoinSet::new();
                    let start = Instant::now();

                    for i in 0..concurrent {
                        let map_clone = Arc::clone(&map);
                        let stream_names = Arc::clone(&stream_names);
                        set.spawn(async move {
                            for y in 0..iters {
                                {
                                    let segment = y%segments;
                                    let key = if segment == 0 {
                                        format!("/{}/0/init.m4s", &stream_names[i])
                                    } else {
                                        format!("/{}/0/{}.m4s", &stream_names[i], segment)
                                    };

                                    let shard = shard(key.as_str(), shards);
                                    let shard_map = map_clone.get(&shard).unwrap();
                                    let mut locked_map = shard_map
                                        .lock()
                                        .unwrap_or_else(|poisoned| poisoned.into_inner());

                                    locked_map.entry(key.clone()).or_insert(0);
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
    // Завершаем группу бенчмарков
    group.finish();
}

fn make_map(
    shards: u64,
    streams: u64,
    tracks: u64,
    segments: u64,
) -> (
    Arc<Vec<String>>,
    Arc<FnvHashMap<u64, Mutex<FnvHashMap<String, u64>>>>,
) {
    let mut stream_names = Vec::with_capacity(streams as usize);
    for _ in 0..streams {
        stream_names.push(Uuid::new_v4().to_string());
    }

    let mut map = FnvHashMap::default();
    for i in 0..shards {
        let shard: FnvHashMap<String, u64> = FnvHashMap::default();
        map.insert(i, Mutex::new(shard));
    }

    for stream in &stream_names {
        for t in 0..tracks {
            for s in 0..segments {
                let key = if s == 0 {
                    format!("{}/{}/init.m4s", stream, t)
                } else {
                    format!("{}/{}/{}.m4s", stream, t, s)
                };
                let shard = shard(key.as_str(), shards);
                map.get(&shard).unwrap().lock().unwrap().insert(key, shard);
            }
        }
    }

    (Arc::new(stream_names), Arc::new(map))
}

fn shard(input: &str, shards: u64) -> u64 {
    let mut hasher = FnvHasher::default();
    input.hash(&mut hasher);

    let hash_value = hasher.finish();
    hash_value % shards
}

criterion_group!(benches, hashmap);
criterion_main!(benches);
