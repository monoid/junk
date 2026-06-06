pub use crossbeam_epoch as epoch;
use crossbeam_epoch::Guard;
use crossbeam_queue::SegQueue;
use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

pub const RECORD_SIZE: usize = 64;

pub struct OwningCollector<T> {
    // Collector must be first, so that deferred callbacks are executed before data is dropped.
    collector: epoch::Collector,
    data: T,
}

impl<T> OwningCollector<T> {
    pub fn new(data: T) -> Self {
        Self {
            collector: epoch::Collector::new(),
            data,
        }
    }

    pub fn as_ref(&self) -> &T {
        &self.data
    }

    pub fn register(&self) -> epoch::LocalHandle {
        self.collector.register()
    }

    pub fn pin(&self) -> Guard {
        self.collector.register().pin()
    }

    pub fn into_inner(self) -> T {
        self.data
    }
}

pub struct Slot {
    pub data: UnsafeCell<[u8; RECORD_SIZE]>,
    pub alive: AtomicBool,
}

unsafe impl Sync for Slot {}
unsafe impl Send for Slot {}

struct StorageInner {
    slots: Vec<Slot>,
    freelist: SegQueue<usize>,
}

pub struct Storage {
    inner: OwningCollector<StorageInner>,
    len: AtomicUsize,          // logical end (только растёт)
}

impl Storage {
    pub fn new(capacity: usize) -> Self {
        let mut slots = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            slots.push(Slot {
                data: UnsafeCell::new([0u8; RECORD_SIZE]),
                alive: AtomicBool::new(false),
            });
        }
        Self {
            inner: OwningCollector::new(StorageInner {
                slots,
                freelist: SegQueue::new(),
            }),
            len: AtomicUsize::new(0),
        }
    }

    /// Запись. Guard нужен чтобы freelist.pop() видел только безопасные слоты.
    pub fn insert(&self, data: &[u8; RECORD_SIZE], _guard: &Guard) -> usize {
        let index = if let Some(idx) = self.inner.as_ref().freelist.pop() {
            idx
        } else {
            self.len.fetch_add(1, Ordering::Relaxed)
        };

        unsafe {
            (*self.inner.as_ref().slots[index].data.get()).copy_from_slice(data);
        }
        self.inner.as_ref().slots[index].alive.store(true, Ordering::Release);
        index
    }

    /// Удаление. Слот уйдёт в freelist только после того, как все
    /// текущие читатели (держащие Guard) завершат свою эпоху.
    pub fn delete(&self, index: usize, guard: &Guard) {
        // Логически удаляем сразу — новые читатели пропустят слот
        self.inner.as_ref().slots[index].alive.store(false, Ordering::Release);

        // Физически возвращаем в freelist — отложено до смены эпохи
        let freelist = &self.inner.as_ref().freelist as *const SegQueue<usize>;
        unsafe {
            guard.defer_unchecked(move || {
                (*freelist).push(index);
            });
        }
        // Можно не вызывать flush() — эпоха сдвинется сама при следующем pin/unpin
    }

    /// Последовательное чтение. Pin гарантирует, что удалённые в процессе
    /// чтения слоты не попадут в freelist и не будут перезаписаны.
    pub fn scan(&self, mut f: impl FnMut(usize, &[u8; RECORD_SIZE])) {
        let guard = self.inner.pin();
        let len = self.len.load(Ordering::Acquire);

        for i in 0..len {
            // Acquire: если видим true, гарантированно видим записанные данные
            if self.inner.as_ref().slots[i].alive.load(Ordering::Acquire) {
                let data = unsafe { &*self.inner.as_ref().slots[i].data.get() };
                f(i, data);
            }
        }

        drop(guard); // явно — здесь эпоха может продвинуться
    }

    pub fn len(&self) -> usize {
        self.len.load(Ordering::Acquire)
    }

    pub fn pin(&self) -> Guard {
        self.inner.pin()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::RngExt as _;

    #[test]
    fn test_basic() {
        use rayon::prelude::*;

        const SIZE: usize = 640;
        const COUNT: usize = 100;

        (0..1000).into_par_iter().for_each(|_| {
            let storage = Storage::new(SIZE);

            {
                let guard = storage.pin();
                for i in 0..SIZE / 20 {
                    storage.insert(&[i as u8; 64], &guard);
                }
            }

            std::thread::scope(|s| {
                s.spawn(|| {
                    let guard = storage.pin();
                    let data = [0u8; 64];
                    for _ in 0..100 {
                        storage.insert(&data, &guard);
                    }
                });

                s.spawn(|| {
                    let mut rnd = rand::rng();
                    for _ in 0..COUNT {
                        let idx = rnd.random_range(0..SIZE);
                        let guard = storage.pin();
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
                        std::hint::black_box(sum);
                    }
                });
            });
        });
    }
}
