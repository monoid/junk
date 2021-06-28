use std::cell::Cell;

/// Accumulate GC stats.  This trait has inferior mutability by design.
pub trait Stat {
    fn gc(&self);
    fn alloc(&self, size: usize);
    fn move_obj(&self, size: usize);
    fn fetch_stat(&self) -> StatInfo;
}

/// GC stat result.
#[derive(Clone, Default)]
pub struct StatInfo {
    pub gc_count: usize,
    pub alloc_size: usize,
    pub alloc_count: usize,
    pub move_size: usize,
    pub move_count: usize,
}

/// Simple single-threaded stats.
#[derive(Default)]
pub struct SimpleStat {
    gc_count: Counter,
    alloc_size: Counter,
    alloc_count: Counter,
    move_size: Counter,
    move_count: Counter,
}

#[derive(Default)]
struct Counter {
    cnt: Cell<usize>,
}

impl Counter {
    fn inc(&self) {
        self.cnt.set(self.cnt.get() + 1);
    }

    fn add(&self, val: usize) {
        self.cnt.set(self.cnt.get() + val);
    }

    fn take(&self) -> usize {
        self.cnt.take()
    }
}

impl Stat for SimpleStat {
    fn gc(&self) {
        self.gc_count.inc()
    }

    fn alloc(&self, size: usize) {
        self.alloc_size.add(size);
        self.alloc_count.inc();
    }

    fn move_obj(&self, size: usize) {
        self.move_size.add(size);
        self.move_count.inc();
    }

    fn fetch_stat(&self) -> StatInfo {
        StatInfo {
            gc_count: self.gc_count.take(),
            alloc_size: self.alloc_size.take(),
            alloc_count: self.alloc_count.take(),
            move_size: self.move_size.take(),
            move_count: self.move_count.take(),
        }
    }
}

/// Null stats that does nothing.
pub struct NullStat {}

impl Stat for NullStat {
    fn gc(&self) {}

    fn alloc(&self, _size: usize) {}

    fn move_obj(&self, _size: usize) {}

    fn fetch_stat(&self) -> StatInfo {
        Default::default()
    }
}
