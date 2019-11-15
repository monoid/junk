use std::sync::atomic::{AtomicBool, Ordering};
use std::cell::UnsafeCell;
use std::ops::{Deref, DerefMut};

pub struct AtomicMutex<T> {
    value: UnsafeCell<T>,
    atomlock: AtomicBool,
}

pub struct AtomicMutexGuard<'a, T> {
    parent: &'a AtomicMutex<T>
}

impl<T> AtomicMutex<T> {
    pub fn new(value: T) -> AtomicMutex<T> {
	AtomicMutex::<T> {
	    value: UnsafeCell::<T>::new(value),
	    atomlock: AtomicBool::new(false),
	}
    }

    pub fn lock<'a>(self: &'a AtomicMutex<T>) -> AtomicMutexGuard<'a, T> {
	while self.atomlock.compare_and_swap(false, true, Ordering::AcqRel) {
	}
	AtomicMutexGuard {
	    parent: self
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
}
