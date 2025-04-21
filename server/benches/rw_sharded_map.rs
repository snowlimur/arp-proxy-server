extern crate gperftools;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use gperftools::profiler::PROFILER;
use rand::Rng;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, RwLock};
use std::time::Instant;
use tokio::runtime::Builder;
use tokio::task::JoinSet;

fn rw_sharded_hashmap(c: &mut Criterion) {
    let core_count = num_cpus::get();
    let shards_configs = [1, 2, 10, 50];
    for num_shards in shards_configs {
        PROFILER
            .lock()
            .unwrap()
            .start(format!("./rw_sharded_hashmap-{}.cpu.prof", num_shards))
            .unwrap();

        bench(c, num_shards, core_count);

        PROFILER.lock().unwrap().stop().unwrap();
    }
}

fn bench(c: &mut Criterion, shards: u64, num_threads: usize) {
    let runtime = Builder::new_multi_thread()
        .worker_threads(num_threads)
        .enable_all()
        .build()
        .expect("Не удалось создать Tokio Runtime");

    // Группа бенчмарков для лучшей организации в отчетах
    let group_name = format!("RW-HashMap-Sharded-{}-{}", shards, num_threads);
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
                                for y in 0..iters {
                                    {
                                        let shard = shard(key.as_str(), shards);
                                        let shard_map = map_clone.get(&shard).unwrap();
                                        let mut found = false;
                                        {
                                            let read_lock =
                                                shard_map.read().unwrap_or_else(|poisoned| {
                                                    // Обработка случая, если мьютекс "отравлен" (другой поток запаниковал)
                                                    // В бенчмарке можно просто паниковать или вернуть данные из отравленного мьютекса
                                                    poisoned.into_inner()
                                                });

                                            let val = read_lock.get(&key);
                                            if val.is_some() {
                                                found = true;
                                            }
                                        }

                                        // Эмулируем промах по кэшу
                                        // (y % 10) >= 6 = 60% Hit Ratio
                                        // (y % 10) >= 7 = 70% Hit Ratio
                                        // (y % 10) >= 8 = 80% Hit Ratio
                                        // (y % 10) >= 9 = 90% Hit Ratio
                                        if !found || (y % 10) >= 9 {
                                            // Блокируем мьютекс для доступа к карте
                                            let mut write_lock =
                                                shard_map.write().unwrap_or_else(|poisoned| {
                                                    // Обработка случая, если мьютекс "отравлен" (другой поток запаниковал)
                                                    // В бенчмарке можно просто паниковать или вернуть данные из отравленного мьютекса
                                                    poisoned.into_inner()
                                                });

                                            // Получаем значение или вставляем 0, если ключа нет,
                                            write_lock.entry(key.clone()).or_insert(0);
                                        }
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

fn make_map(shards: u64, fill: u64) -> Arc<HashMap<u64, RwLock<HashMap<String, u64>>>> {
    let mut map = HashMap::new();
    for i in 0..shards {
        let mut shard: HashMap<String, u64> = HashMap::new();
        for y in 0..(fill / shards) {
            let key = gen_key(i, 10, y);
            shard.insert(key, i);
        }

        map.insert(i, RwLock::new(shard));
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

fn hit(probability_percent: u8) -> bool {
    // Ограничиваем вероятность диапазоном [0, 100]
    let effective_probability = probability_percent.min(100);

    // Если вероятность 0%, всегда возвращаем false
    if effective_probability == 0 {
        return false;
    }

    // Если вероятность 100%, всегда возвращаем true
    if effective_probability == 100 {
        return true;
    }

    // Получаем генератор случайных чисел для текущего потока
    let mut rng = rand::rng();

    // Генерируем случайное число от 0 до 99 включительно
    let random_number = rng.random_range(0..100);

    // Сравниваем случайное число с порогом вероятности
    // Если число меньше порога, возвращаем true
    random_number < effective_probability
}

criterion_group!(benches, rw_sharded_hashmap);
criterion_main!(benches);
