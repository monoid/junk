pub use crossbeam_epoch as epoch;
use crossbeam_epoch::Guard;
use crossbeam_queue::SegQueue;
use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

const RECORD_SIZE: usize = 64;

struct Slot {
    data: UnsafeCell<[u8; RECORD_SIZE]>,
    alive: AtomicBool,
}

unsafe impl Sync for Slot {}
unsafe impl Send for Slot {}

pub struct Storage {
    slots: Vec<Slot>,
    len: AtomicUsize,          // logical end (только растёт)
    freelist: SegQueue<usize>, // индексы, безопасные для переиспользования
}

// impl Drop for Storage {
//     fn drop(&mut self) {
//         crossbeam_epoch::pin().repin();
//     }
// }

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
            slots,
            len: AtomicUsize::new(0),
            freelist: SegQueue::new(),
        }
    }

    /// Запись. Guard нужен чтобы freelist.pop() видел только безопасные слоты.
    pub fn insert(&self, data: &[u8; RECORD_SIZE], _guard: &Guard) -> usize {
        let index = if let Some(idx) = self.freelist.pop() {
            idx
        } else {
            self.len.fetch_add(1, Ordering::Relaxed)
        };

        unsafe {
            (*self.slots[index].data.get()).copy_from_slice(data);
        }
        self.slots[index].alive.store(true, Ordering::Release);
        index
    }

    /// Удаление. Слот уйдёт в freelist только после того, как все
    /// текущие читатели (держащие Guard) завершат свою эпоху.
    pub fn delete(&self, index: usize, guard: &Guard) {
        // Логически удаляем сразу — новые читатели пропустят слот
        self.slots[index].alive.store(false, Ordering::Release);

        // Физически возвращаем в freelist — отложено до смены эпохи
        let freelist = &self.freelist as *const SegQueue<usize>;
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
        let guard = epoch::pin();
        let len = self.len.load(Ordering::Acquire);

        for i in 0..len {
            // Acquire: если видим true, гарантированно видим записанные данные
            if self.slots[i].alive.load(Ordering::Acquire) {
                let data = unsafe { &*self.slots[i].data.get() };
                f(i, data);
            }
        }

        drop(guard); // явно — здесь эпоха может продвинуться
    }

    pub fn len(&self) -> usize {
        self.len.load(Ordering::Acquire)
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
                let guard = epoch::pin();
                for i in 0..SIZE / 20 {
                    storage.insert(&[i as u8; 64], &guard);
                }
            }

            std::thread::scope(|s| {
                s.spawn(|| {
                    let guard = epoch::pin();
                    let data = [0u8; 64];
                    for _ in 0..100 {
                        storage.insert(&data, &guard);
                    }
                });

                s.spawn(|| {
                    let mut rnd = rand::rng();
                    for _ in 0..COUNT {
                        let idx = rnd.random_range(0..SIZE);
                        let guard = epoch::pin();
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
