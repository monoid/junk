use criterion::*;
use crossby::{RECORD_SIZE, Slot, Storage};
use parking_lot::RwLock;
use rand::RngExt as _;
use std::cell::UnsafeCell;
use std::hint::black_box;
use std::sync::atomic::{AtomicBool, Ordering};

const READES: usize = 20;

pub fn basic_bench(c: &mut Criterion) {
    let size = 640;
    const COUNT: usize = 1000;

    c.bench_function("epoch", |b| {
        b.iter(|| {
            let storage = Storage::new(size);
            let handle = storage.register();

            {
                let guard = handle.pin();
                for i in 0..size / 20 {
                    storage.insert(&[i as u8; 64], &guard);
                }
            }

            std::thread::scope(|s| {
                s.spawn(|| {
                    let handle = storage.register();
                    let data = [0u8; 64];
                    for _ in 0..size - size / 20 {
                        let guard = handle.pin();
                        storage.insert(&data, &guard);
                    }
                });

                s.spawn(|| {
                    let mut rnd = rand::rng();
                    let handle = storage.register();
                    for _ in 0..10 * COUNT {
                        let idx = rnd.random_range(0..size);
                        let guard = handle.pin();
                        storage.delete(idx, &guard);
                    }
                });

                for _ in 0..READES {
                    s.spawn(|| {
                        let handle = storage.register();
                        for _ in 0..COUNT {
                            let mut sum = 0;
                            storage.scan(
                                |_, data| {
                                    for byte in data {
                                        sum += *byte as usize;
                                    }
                                },
                                &handle,
                            );
                            black_box(sum);
                        }
                    });
                }
            });

            handle.pin().flush();
            std::mem::drop(storage);
        })
    });
}

struct StorageRwLock {
    inner: RwLock<StorageRwLockInner>,
}

struct StorageRwLockInner {
    slots: Vec<Slot>,
    freelist: Vec<usize>,
    len: usize,
}
impl StorageRwLock {
    pub fn new(capacity: usize) -> Self {
        let mut slots = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            slots.push(Slot {
                data: UnsafeCell::new([0u8; RECORD_SIZE]),
                alive: AtomicBool::new(false),
            });
        }
        let inner = StorageRwLockInner {
            slots,
            freelist: Vec::new(),
            len: 0,
        };
        Self {
            inner: RwLock::new(inner),
        }
    }

    pub fn insert(&self, data: &[u8; RECORD_SIZE]) -> usize {
        let mut guard = self.inner.write();

        let index = if let Some(idx) = guard.freelist.pop() {
            idx
        } else {
            let idx = guard.len;
            guard.len += 1;
            idx
        };

        unsafe {
            (*guard.slots[index].data.get()).copy_from_slice(data);
        }
        guard.slots[index].alive.store(true, Ordering::Relaxed);
        index
    }

    pub fn delete(&self, index: usize) {
        let mut guard = self.inner.write();

        // Логически удаляем сразу — новые читатели пропустят слот
        guard.slots[index].alive.store(false, Ordering::Relaxed);
        guard.freelist.push(index);
    }

    pub fn scan(&self, mut f: impl FnMut(usize, &[u8; RECORD_SIZE])) {
        let guard = self.inner.read();
        let len = guard.len;

        for i in 0..len {
            // Acquire: если видим true, гарантированно видим записанные данные
            if guard.slots[i].alive.load(Ordering::Relaxed) {
                let data = unsafe { &*guard.slots[i].data.get() };
                f(i, data);
            }
        }
    }

    pub fn len(&self) -> usize {
        self.inner.read().len
    }
}

fn mutex_bench(c: &mut Criterion) {
    let size = 640;
    const COUNT: usize = 1000;

    c.bench_function("mutex", |b| {
        b.iter(|| {
            let storage = StorageRwLock::new(size);

            for i in 0..size / 20 {
                storage.insert(&[i as u8; 64]);
            }

            std::thread::scope(|s| {
                s.spawn(|| {
                    for i in 0..size - size / 20 {
                        storage.insert(&[i as u8; 64]);
                    }
                });

                s.spawn(|| {
                    let mut rnd = rand::rng();
                    for _ in 0..10 * COUNT {
                        let len = storage.len();
                        let idx = rnd.random_range(0..len);
                        storage.delete(idx);
                    }
                });

                for _ in 0..READES {
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
                }
            });

            std::mem::drop(storage);
        })
    });
}

criterion_group!(benches, basic_bench, mutex_bench);
criterion_main!(benches);
