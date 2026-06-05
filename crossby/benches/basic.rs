use criterion::*;
use crossby::Storage;
use rand::RngExt as _;
use std::hint::black_box;

pub fn basic_bench(c: &mut Criterion) {
    let size = 640;
    const COUNT: usize = 100;

    c.bench_function("basic", |b| {
        b.iter(|| {
            let storage = Storage::new(size);

            {
                let guard = crossby::epoch::pin();
                for i in 0..size / 20 {
                    storage.insert(&[i as u8; 64], &guard);
                }
            }

            std::thread::scope(|s| {
                s.spawn(|| {
                    let guard = crossby::epoch::pin();
                    let data = [0u8; 64];
                    for _ in 0..100 {
                        storage.insert(&data, &guard);
                    }
                });

                s.spawn(|| {
                    let mut rnd = rand::rng();
                    for _ in 0..COUNT {
                        let idx = rnd.random_range(0..size);
                        let guard = crossby::epoch::pin();
                        storage.delete(idx, &guard);
                    }
                });

                s.spawn(|| {
                    for _ in 0..COUNT {
                        let mut sum = 0;
                        storage.scan(|_, data| {
                            for byte in data {
                                sum += *byte as usize;
                            }
                        });
                        black_box(sum);
                    }
                });
            });

            crossby::epoch::pin().flush();
            std::mem::drop(storage);
        })
    });
}

criterion_group!(benches, basic_bench);
criterion_main!(benches);
