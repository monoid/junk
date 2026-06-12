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
    len: AtomicUsize, // logical end (только растёт)
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

    pub fn insert(&self, data: &[u8; RECORD_SIZE], _guard: &Guard) -> usize {
        let index = if let Some(idx) = self.inner.as_ref().freelist.pop() {
            idx
        } else {
            // We do not check capacity for simplicity.
            self.len.fetch_add(1, Ordering::Relaxed)
        };

        unsafe {
            (*self.inner.as_ref().slots[index].data.get()).copy_from_slice(data);
        }
        self.inner.as_ref().slots[index]
            .alive
            .store(true, Ordering::Release);
        index
    }

    pub fn delete(&self, index: usize, guard: &Guard) -> bool {
        // Immediately remove logically: new readers will skip the slot.
        if self.inner.as_ref().slots[index].alive.compare_exchange(
            true,
            false,
            Ordering::Release,
            Ordering::Relaxed,
        ).is_err() {
            // Already deleted, do nothing.
            return false;
        }

        // Adding to the freelist is deferred until change of epoch.
        let freelist = &self.inner.as_ref().freelist as *const SegQueue<usize>;
        unsafe {
            guard.defer_unchecked(move || {
                (*freelist).push(index);
            });
        }
        true
    }

    /// Последовательное чтение. Pin гарантирует, что удалённые в процессе
    /// чтения слоты не попадут в freelist и не будут перезаписаны.
    pub fn scan(&self, mut f: impl FnMut(usize, &[u8; RECORD_SIZE]), handle: &epoch::LocalHandle) {
        let guard = handle.pin();
        let len = self.len.load(Ordering::Acquire);

        for i in 0..len {
            // Acquire: if it is true, we are guaranteed to see the valid data
            if self.inner.as_ref().slots[i].alive.load(Ordering::Acquire) {
                let data = unsafe { &*self.inner.as_ref().slots[i].data.get() };
                f(i, data);
            }
        }

        drop(guard);
    }

    pub fn len(&self) -> usize {
        self.len.load(Ordering::Acquire)
    }

    pub fn register(&self) -> epoch::LocalHandle {
        self.inner.register()
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
                let handle = storage.register();
                let guard = handle.pin();
                for i in 0..SIZE / 20 {
                    storage.insert(&[i as u8; 64], &guard);
                }
            }

            std::thread::scope(|s| {
                s.spawn(|| {
                    let handle = storage.register();
                    for i in 0..100 {
                        let guard = handle.pin();
                        let data = [i as u8; 64];
                        storage.insert(&data, &guard);
                    }
                });

                s.spawn(|| {
                    let handle = storage.register();
                    let mut rnd = rand::rng();
                    for _ in 0..COUNT {
                        let idx = rnd.random_range(0..SIZE);
                        let guard = handle.pin();
                        storage.delete(idx, &guard);
                    }
                });

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
                        std::hint::black_box(sum);
                    }
                });
            });
        });
    }
}
