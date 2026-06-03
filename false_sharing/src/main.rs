mod params;

use std::ops::Deref;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use params::{N, SIZE};
const ORDERING: Ordering = Ordering::AcqRel;

#[cfg_attr(feature = "cache_line_64", repr(align(64)))]
#[cfg_attr(feature = "cache_line_128", repr(align(128)))]
#[derive(Default, Debug)]
struct CachePadded<T> {
    value: T,
}

impl<T> Deref for CachePadded<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

// Two atomics side-by-side separated by cache padding configured above.
#[derive(Default, Debug)]
struct AddSub {
    // Updated by the first thread.
    add: CachePadded<AtomicU64>,
    // Updated by the second thread.
    sub: CachePadded<AtomicU64>,
}

impl AddSub {
    fn get_value(&self) -> u64 {
        let add = self.add.load(Ordering::Relaxed);
        let sub = self.sub.load(Ordering::Relaxed);
        add.wrapping_sub(sub)
    }
}

fn main() {
    let data = Vec::from_iter(std::iter::repeat_with(|| AddSub::default()).take(SIZE));
    let addsub: Box<[AddSub]> = data.into();
    let addsub = &*Box::leak(addsub);

    let t1 = std::thread::spawn(move || {
        #[cfg(target_arch = "x86_64")]
        affinity::set_thread_affinity(&[0]).unwrap();

        for _ in 0..(N / SIZE) {
            for elt in addsub {
                elt.add.fetch_add(1 as u64, ORDERING);
            }
        }
    });
    let t2 = std::thread::spawn(move || {
        #[cfg(target_arch = "x86_64")]
        // Not 1! Core 1 is a virtual core of the same physical one.
        affinity::set_thread_affinity(&[2]).unwrap();

        for _ in 0..(N / SIZE) {
            for elt in addsub {
                elt.sub.fetch_add(1 as u64, ORDERING);
            }
        }
    });
    t1.join().unwrap();
    t2.join().unwrap();

    for item in &*addsub {
        eprintln!("{}", item.get_value());
    }
}
