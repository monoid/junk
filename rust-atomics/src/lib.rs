use std::cell::UnsafeCell;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{spin_loop_hint, AtomicBool, AtomicI32, Ordering};

///
/// Mutex
///
pub struct AtomicMutex<T> {
    value: UnsafeCell<T>,
    atomlock: AtomicBool,
}

unsafe impl<T: Send> Send for AtomicMutex<T> {}

unsafe impl<T: Send> Sync for AtomicMutex<T> {}

pub struct AtomicMutexGuard<'a, T> {
    parent: &'a AtomicMutex<T>,
}

impl<T> AtomicMutex<T> {
    pub fn new(value: T) -> Self {
        Self {
            value: UnsafeCell::<T>::new(value),
            atomlock: AtomicBool::new(false),
        }
    }

    pub fn lock(&self) -> AtomicMutexGuard<T> {
        while self.atomlock.swap(true, Ordering::AcqRel) {
            spin_loop_hint()
        }
        AtomicMutexGuard { parent: self }
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
        unsafe { &*self.parent.value.get() }
    }
}

impl<T> DerefMut for AtomicMutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.parent.value.get() }
    }
}

///
/// RwLock
///
pub struct AtomicRwLock<T> {
    value: UnsafeCell<T>,
    writerlock: AtomicBool,
    readercount: AtomicI32,
}

unsafe impl<T: Send> Send for AtomicRwLock<T> {}

unsafe impl<T: Send> Sync for AtomicRwLock<T> {}

pub struct AtomicReaderRwLockGuard<'a, T> {
    parent: &'a AtomicRwLock<T>,
}

pub struct AtomicWriterRwLockGuard<'a, T> {
    parent: &'a AtomicRwLock<T>,
}

impl<T> AtomicRwLock<T> {
    pub fn new(value: T) -> Self {
        Self {
            value: UnsafeCell::new(value),
            writerlock: AtomicBool::new(false),
            readercount: AtomicI32::new(0),
        }
    }

    pub fn readlock(&self) -> AtomicReaderRwLockGuard<T> {
        loop {
            while self.writerlock.load(Ordering::Acquire) {
                spin_loop_hint()
            }
            self.readercount.fetch_add(1, Ordering::SeqCst);
            if self.writerlock.load(Ordering::Acquire) {
                // Rollback increment and try again
                self.readercount.fetch_sub(1, Ordering::SeqCst);
                continue;
            } else {
                return AtomicReaderRwLockGuard { parent: self };
            }
        }
    }

    pub fn writelock(&self) -> AtomicWriterRwLockGuard<T> {
        while self.writerlock.swap(true, Ordering::AcqRel) {
            spin_loop_hint()
        }
        while self.readercount.load(Ordering::Acquire) != 0 {
            spin_loop_hint()
        }
        AtomicWriterRwLockGuard { parent: self }
    }
}

impl<T> Deref for AtomicReaderRwLockGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.parent.value.get() }
    }
}

impl<T> Deref for AtomicWriterRwLockGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.parent.value.get() }
    }
}

impl<T> DerefMut for AtomicWriterRwLockGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.parent.value.get() }
    }
}

impl<T> Drop for AtomicReaderRwLockGuard<'_, T> {
    fn drop(&mut self) {
        self.parent.readercount.fetch_sub(1, Ordering::SeqCst);
    }
}

impl<T> Drop for AtomicWriterRwLockGuard<'_, T> {
    fn drop(&mut self) {
        self.parent.writerlock.store(false, Ordering::SeqCst);
    }
}

///
/// Tests
///
#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;
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
        const REPEAT: i32 = 10000000;
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

    #[test]
    fn it_works_rwlock() {
        let mutex = AtomicRwLock::<i32>::new(1);

        {
            let guard = mutex.readlock();
            assert_eq!(*guard, 1);
        }

        {
            let mut guard = mutex.writelock();
            *guard = 2;
            assert_eq!(*guard, 2);
        }

        {
            let guard = mutex.readlock();
            assert_eq!(*guard, 2);
        }
    }

    #[test]
    #[ignore]
    fn test_rwlock() {
        struct Data {
            a: i32,
            b: i32,
        }
        let stop_flag = Arc::new(AtomicBool::new(false));
        let (stop1, stop2) = (stop_flag.clone(), stop_flag.clone());

        const REPEAT: i32 = 100000000;
        let v = Arc::new(AtomicRwLock::<Data>::new(Data { a: 0, b: 0 }));

        let (v1, v2, v3, v4) = (v.clone(), v.clone(), v.clone(), v.clone());
        let wth1 = thread::spawn(move || {
            for _i in 0..REPEAT {
                let mut guard = v1.writelock();
                guard.a += 1;
                guard.b += 1;
            }
        });
        let wth2 = thread::spawn(move || {
            for _i in 0..REPEAT {
                let mut guard = v2.writelock();
                guard.a += 1;
                guard.b += 1;
            }
        });

        let rth1 = thread::spawn(move || {
            for _i in 0..REPEAT {
                let guard = v3.readlock();
                assert_eq!(guard.a, guard.b);
                if stop1.load(Ordering::Relaxed) {
                    break;
                }
            }
        });
        let rth2 = thread::spawn(move || {
            for _i in 0..REPEAT {
                let guard = v4.readlock();
                assert_eq!(guard.a, guard.b);
                if stop2.load(Ordering::Relaxed) {
                    break;
                }
            }
        });

        wth1.join().unwrap();
        wth2.join().unwrap();

        stop_flag.store(false, Ordering::Release);

        rth1.join().unwrap();
        rth2.join().unwrap();

        let guard = v.readlock();
        assert_eq!(guard.a, 2 * REPEAT);
        assert_eq!(guard.b, 2 * REPEAT);
    }

    /// Struct that increments counter on creation and decrements on drop.
    struct Counted {
        counter: Rc<RefCell<i32>>,
    }

    impl Counted {
        fn new(counter: &Rc<RefCell<i32>>) -> Self {
            let counter1 = counter.clone();
            *counter1.borrow_mut() += 1;

            Counted { counter: counter1 }
        }
    }

    impl Drop for Counted {
        fn drop(&mut self) {
            *self.counter.borrow_mut() -= 1;
        }
    }

    #[test]
    fn test_mutex_drop() {
        let counter = Rc::new(RefCell::new(0));

        assert_eq!(*counter.borrow(), 0);

        let counted = Counted::new(&counter);

        assert_eq!(*counter.borrow(), 1);
        {
            let _mutex = AtomicMutex::new(counted);

            assert_eq!(*counter.borrow(), 1);
        }

        assert_eq!(*counter.borrow(), 0);
    }

    #[test]
    fn test_rwlock_drop() {
        let counter = Rc::new(RefCell::new(0));

        assert_eq!(*counter.borrow(), 0);

        let counted = Counted::new(&counter);

        assert_eq!(*counter.borrow(), 1);
        {
            let _mutex = AtomicRwLock::new(counted);

            assert_eq!(*counter.borrow(), 1);
        }

        assert_eq!(*counter.borrow(), 0);
    }
}
