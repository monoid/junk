use std::sync::atomic::{AtomicBool, Ordering, spin_loop_hint};
use std::cell::UnsafeCell;
use std::ops::{Deref, DerefMut};

pub struct AtomicMutex<T> {
    value: UnsafeCell<T>,
    atomlock: AtomicBool,
}

unsafe impl<T: Send> Send for AtomicMutex<T> {
}

unsafe impl<T: Send> Sync for AtomicMutex<T> {
}

pub struct AtomicMutexGuard<'a, T> {
    parent: &'a AtomicMutex<T>
}

impl<T> AtomicMutex<T> {
    pub fn new(value: T) -> Self {
        Self {
            value: UnsafeCell::<T>::new(value),
            atomlock: AtomicBool::new(false),
        }
    }

    pub fn lock<'a>(self: &'a Self) -> AtomicMutexGuard<'a, T> {
        while self.atomlock.swap(true, Ordering::AcqRel) {
            spin_loop_hint()
        }
        AtomicMutexGuard {
            parent: self
        }
    }
}

impl<T> Drop for AtomicMutex<T> {
    fn drop(&mut self) {
        unsafe {
            self.value.get().drop_in_place()
        }
    }
}

impl<T> Drop for AtomicMutexGuard<'_, T> {
    fn drop(&mut self) {
        self.parent.atomlock.store(false, Ordering::Release);
    }
}

impl<T> Deref for AtomicMutexGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe {
            & *self.parent.value.get()
        }
    }
}
impl<T> DerefMut for AtomicMutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe {
            &mut *self.parent.value.get()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
        let mutex = AtomicMutex::<i32>::new(1);
        assert!(!mutex.atomlock.load(Ordering::Acquire));

        {
            let guard = mutex.lock();
            assert!(mutex.atomlock.load(Ordering::Acquire));
            assert_eq!(*guard, 1);
        }

        {
            let mut guard = mutex.lock();
            assert!(mutex.atomlock.load(Ordering::Acquire));
            *guard = 2;
            assert_eq!(*guard, 2);
        }

        {
            let guard = mutex.lock();
            assert!(mutex.atomlock.load(Ordering::Acquire));
            assert_eq!(*guard, 2);
        }

        assert!(!mutex.atomlock.load(Ordering::Acquire));
    }

    #[test]
    fn test_arc() {
        const REPEAT : i32 = 10000000;
        let v = Arc::new(AtomicMutex::<i32>::new(0));

        let (v1, v2) = (v.clone(), v.clone());
        let th1 = thread::spawn(move || {
            for _i in 0..REPEAT {
                let mut guard = v1.lock();
                *guard += 1;
            }
        });
        let th2 = thread::spawn(move || {
            for _i in 0..REPEAT {
                let mut guard = v2.lock();
                *guard += 1;
            }
        });
        th1.join().unwrap();
        th2.join().unwrap();

        let guard = v.lock();
        assert_eq!(*guard, 2 * REPEAT);
    }
}
