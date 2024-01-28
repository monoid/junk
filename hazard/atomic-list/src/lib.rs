use std::{
    ops::Deref,
    sync::atomic::{AtomicPtr, AtomicUsize, Ordering},
};

// TODO Actually, we need a geterogenious list.
#[derive(Default)]
pub struct RawNode {
    pub next: AtomicPtr<RawNode>,
    pub value: AtomicUsize,
}

pub struct NodeGuard<'list> {
    node: &'list RawNode,
}

// TODO with this, we may store zero to a pointer, and it will be hijacked.
// Is it even a problem?  Sure it is.
impl Deref for NodeGuard<'_> {
    type Target = AtomicUsize;

    fn deref(&self) -> &Self::Target {
        &self.node.value
    }
}

impl Drop for NodeGuard<'_> {
    fn drop(&mut self) {
        self.node.value.store(0, Ordering::Release);
    }
}

/// Grow-only linked list.
pub struct LockFreeList {
    next: AtomicPtr<RawNode>,
    len: AtomicUsize,
}

impl LockFreeList {
    pub fn new() -> Self {
        Self {
            next: <_>::default(),
            len: <_>::default(),
        }
    }

    pub fn len(&self) -> usize {
        self.len.load(Ordering::Relaxed)
    }

    pub fn get_node<T>(&self, value: *const T) -> Option<NodeGuard<'_>> {
        if value.is_null() {
            // null is a special value in the list, and it doesn't need a deallocation.
            return None;
        }
        let value = value as _; // Change to mut pointer to conform to AtomicPtr API.

        unsafe {
            let mut node = self.next.load(Ordering::Acquire).as_ref();
            while let Some(n) = node {
                if n.value
                    .compare_exchange(0, value, Ordering::Release, Ordering::Relaxed)
                    .is_ok()
                {
                    return Some(NodeGuard {
                        node: n,
                    });
                } else {
                    node = n.next.load(Ordering::Acquire).as_ref();
                }
            }
        }
        Some(self.insert_fresh_node(value))
    }

    fn insert_fresh_node(&self, value: usize) -> NodeGuard<'_> {
        let node = Box::new(RawNode {
            next: AtomicPtr::default(),
            value: value.into(),
        });

        self.len.fetch_add(1, Ordering::Relaxed);

        let mut root = self.next.load(Ordering::Acquire);
        loop {
            node.next.store(root, Ordering::Relaxed);

            match self.next.compare_exchange_weak(
                root,
                node.as_ref() as *const RawNode as _,
                Ordering::Release,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    let node = Box::into_raw(node);
                    return unsafe {
                        NodeGuard {
                            node: &*node,
                        }
                    };
                }
                Err(fresh_root) => root = fresh_root,
            };
        }
    }

    pub fn iter(&self) -> LockFreeListIter<'_> {
        LockFreeListIter {
            node: unsafe { self.next.load(Ordering::Acquire).as_ref() },
        }
    }
}

pub struct LockFreeListIter<'list> {
    node: Option<&'list RawNode>,
}

impl Iterator for LockFreeListIter<'_> {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        match self.node {
            Some(node) => {
                let val = node.value.load(Ordering::Acquire);
                // The next field doesn't change, and we have used `Aquire` before.
                self.node = unsafe { node.next.load(Ordering::Relaxed).as_ref() };
                Some(val)
            }
            None => None,
        }
    }
}

impl Drop for LockFreeList {
    fn drop(&mut self) {
        let mut next;
        // We use relaxed ordering because we have unique reference
        // (i.e. exlusive access) to the list and its nodes.
        while {
            next = self.next.load(Ordering::Relaxed);
            !next.is_null()
        } {
            // Safety: it is safe because the node is allocated with Box.
            let current = unsafe { Box::from_raw(next) };
            let new_next = current.next.load(Ordering::Relaxed);
            self.next.store(new_next, Ordering::Relaxed);
            // Now drop the `current` box:
            // ...
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn test_simple() {
        let a = LockFreeList::new();
        let v1: isize = 42;
        let v2: isize = 13;
        let n1 = a.get_node::<isize>(&v1 as _).unwrap();
        let n2 = a.get_node::<isize>(&v2 as _).unwrap();

        assert_ne!(n1.load(Ordering::Relaxed), n2.load(Ordering::Relaxed));
    }

    #[test]
    fn test_multi() {
        let a = Arc::new(LockFreeList::new());
        let a1 = a.clone();
        let a2 = a.clone();
        let a3 = a.clone();

        let data1 = vec![1isize, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14];
        let data2 = vec![
            10isize, 20, 30, 40, 50, 60, 70, 80, 90, 100, 110, 120, 130, 140,
        ];
        let data3 = vec![
            100isize, 200, 300, 400, 500, 600, 700, 800, 900, 1000, 1100, 1200, 1300, 1400,
        ];

        let t1 = std::thread::spawn(move || {
            let _v: Vec<_> = data1
                .iter()
                .map(|cell| a1.get_node(cell as _).unwrap())
                .collect();
        });

        let t2 = std::thread::spawn(move || {
            let _v: Vec<_> = data2
                .iter()
                .map(|cell| a2.get_node(cell as _).unwrap())
                .collect();
        });

        let t3 = std::thread::spawn(move || {
            let _v: Vec<_> = data3
                .iter()
                .map(|cell| a3.get_node(cell as _).unwrap())
                .collect();
        });

        t1.join().unwrap();
        t2.join().unwrap();
        t3.join().unwrap();

        assert!(a.iter().all(|p| p == 0))
    }

    #[test]
    fn test_multi_reuse() {
        let a = Arc::new(LockFreeList::new());
        let a1 = a.clone();
        let a2 = a.clone();
        let a3 = a.clone();

        let data1 = vec![1isize, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14];
        let data2 = vec![
            10isize, 20, 30, 40, 50, 60, 70, 80, 90, 100, 110, 120, 130, 140,
        ];

        let t1 = std::thread::spawn(move || {
            let _v: Vec<_> = data1
                .iter()
                .map(|cell| a1.get_node(cell as _).unwrap())
                .collect();
        });

        let t2 = std::thread::spawn(move || {
            let _v: Vec<_> = data2
                .iter()
                .map(|cell| a2.get_node(cell as _).unwrap())
                .collect();
        });

        t1.join().unwrap();
        t2.join().unwrap();

        let last_len = a.len();
        assert!(last_len >= 14);
        assert!(last_len <= 28);

        let mut data = vec![
            100isize, 200, 300, 400, 500, 600, 700, 800, 900, 1000, 1100, 1200, 1300, 1400,
        ];
        let _v: Vec<_> = data
            .iter_mut()
            .map(|cell| a3.get_node(cell as _).unwrap())
            .collect();

        assert_eq!(a.len(), last_len);

        assert!(!a.iter().all(|p| p == 0))
    }

    #[test]
    fn test_multi_heavy() {
        const N: usize = 1000000;

        let a = Arc::new(LockFreeList::new());
        let a1 = a.clone();
        let a2 = a.clone();

        let data1 = vec![1isize, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
        let data2 = vec![
            10isize, 20, 30, 40, 50, 60, 70, 80, 90, 100, 110, 120, 130, 140,
        ];

        let t1 = std::thread::spawn(move || {
            for _ in 0..N {
                let _v: Vec<_> = data1
                    .iter()
                    .map(|cell| a1.get_node(cell as _).unwrap())
                    .collect();
            }
        });

        let t2 = std::thread::spawn(move || {
            for _ in 0..N {
                let _v: Vec<_> = data2
                    .iter()
                    .map(|cell| a2.get_node(cell as _).unwrap())
                    .collect();
            }
        });

        t1.join().unwrap();
        t2.join().unwrap();

        let last_len = a.len();
        assert!(last_len >= 15);
        assert!(last_len <= 29);

        assert!(a.iter().all(|p| p == 0));
    }
}
