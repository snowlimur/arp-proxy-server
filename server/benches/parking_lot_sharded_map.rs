extern crate gperftools;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use futures_util::task::SpawnExt;
use parking_lot::Mutex;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::thread;
use std::time::Instant;
use tokio::runtime::Builder;
use tokio::task::JoinSet;

fn parking_log_sharded_hashmap(c: &mut Criterion) {
    let core_count = num_cpus::get();
    let shards_configs = [1, 2, 10, 50];
    for num_shards in shards_configs {
        bench(c, num_shards, core_count);
    }
}

fn bench(c: &mut Criterion, shards: u64, num_threads: usize) {
    let runtime = Builder::new_multi_thread()
        .worker_threads(num_threads)
        .enable_all()
        .build()
        .expect("Не удалось создать Tokio Runtime");

    // Группа бенчмарков для лучшей организации в отчетах
    let group_name = format!("Parking-Lot-HashMap-Sharded-{}-{}", shards, num_threads);
    let mut group = c.benchmark_group(group_name);

    let concurrent_configs = [1, 100, 1000];
    let map = make_map(shards, 10_000);

    // Итерация по разному количеству конкурентных пользователей
    for concurrent in concurrent_configs {
        // Настройка измерения пропускной способности (операций в секунду)
        // Общее количество осмысленных операций обращения к словарю
        group.throughput(Throughput::Elements(concurrent as u64));

        // Запуск бенчмарка для текущего количества пользователей (`user_count`)
        group.bench_with_input(
            BenchmarkId::new("Concurrent", &concurrent), // Идентификатор для отчета
            &concurrent, // Входной параметр (количество конкурентных корутин)
            |b, &n_coroutines| {
                // Используем iter_custom, т.к. нам нужен контроль над запуском
                // потоков и ожиданием их завершения внутри измеряемого блока.
                b.to_async(&runtime).iter_custom(|iters| {
                    let map = Arc::clone(&map);
                    async move {
                        // --- Setup ---
                        let mut set = JoinSet::new();

                        // --- Начало измерения ---
                        let start = Instant::now();

                        // Запускаем N корутин
                        for i in 0..n_coroutines {
                            let map_clone = Arc::clone(&map);
                            set.spawn(async move {
                                // Генерируем уникальный ключ для этого потока
                                let key = gen_key(i as u64, 10, 20);
                                for _ in 0..iters {
                                    {
                                        let shard = shard(key.as_str(), shards);
                                        let shard_map = map_clone.get(&shard).unwrap();

                                        // Блокируем мьютекс для доступа к карте
                                        let mut locked_map = shard_map.lock();

                                        // Получаем значение или вставляем 0, если ключа нет,
                                        locked_map.entry(key.clone()).or_insert(0);
                                    }
                                }
                            });
                        }

                        // Ожидаем завершения всех запущенных потоков
                        set.join_all().await;

                        // --- Конец измерения ---
                        // Возвращаем общее время, затраченное на выполнение работы всеми потоками
                        start.elapsed()
                    }
                });
            },
        );
    }
    // Завершаем группу бенчмарков
    group.finish();
}

fn make_map(shards: u64, fill: u64) -> Arc<HashMap<u64, Mutex<HashMap<String, u64>>>> {
    let mut map = HashMap::new();
    for i in 0..shards {
        let mut shard: HashMap<String, u64> = HashMap::new();
        for y in 0..(fill / shards) {
            let key = gen_key(i, 10, y);
            shard.insert(key, i);
        }

        map.insert(i, Mutex::new(shard));
    }

    Arc::new(map)
}

fn gen_key(stream: u64, track: u8, segment: u64) -> String {
    format!("/stream-{}/{}/{}.m4s", stream, track, segment)
}

fn shard(input: &str, shards: u64) -> u64 {
    let mut hasher = DefaultHasher::new();
    input.hash(&mut hasher);

    let hash_value = hasher.finish();
    hash_value % shards
}

criterion_group!(benches, parking_log_sharded_hashmap);
criterion_main!(benches);
