use std::{
    ops::Deref,
    sync::{Arc, Mutex},
    thread::{spawn, yield_now},
};

use criterion::{black_box, criterion_group, criterion_main, Criterion};
#[cfg(feature = "arrayvec")]
use lock_many::lock_many_arrayvec;
use lock_many::lock_many_vec;

fn run_parallel_lock_many<const THREAD_NUM: usize>(n: u64, swapped: bool) {
    let m1 = Arc::new(Mutex::new(1u64));
    let m2 = Arc::new(Mutex::new(1u64));
    let mut threads = Vec::with_capacity(THREAD_NUM);

    for i in 0..THREAD_NUM {
        let mut m1a = m1.clone();
        let mut m2a = m2.clone();
        threads.push(spawn(move || {
            if swapped && i & 1 == 0 {
                std::mem::swap(&mut m1a, &mut m2a);
            }
            for _ in 0..n {
                let mut g = lock_many_vec(&[m1a.deref(), m2a.deref()]).unwrap();
                let fib = g[0].wrapping_add(*g[1]);
                *g[1] = *g[0];
                *g[0] = fib;
            }
        }));
    }

    threads.into_iter().for_each(|t| {
        t.join().unwrap();
    });
}

#[cfg(feature = "arrayvec")]
fn run_parallel_lock_many_arrayvec<const THREAD_NUM: usize>(n: u64, swapped: bool) {
    let m1 = Arc::new(Mutex::new(1u64));
    let m2 = Arc::new(Mutex::new(1u64));
    let mut threads = Vec::with_capacity(THREAD_NUM);

    for i in 0..THREAD_NUM {
        let mut m1a = m1.clone();
        let mut m2a = m2.clone();
        threads.push(spawn(move || {
            if swapped && i & 1 == 0 {
                std::mem::swap(&mut m1a, &mut m2a);
            }
            for _ in 0..n {
                let mut g =
                    lock_many_arrayvec::<_, THREAD_NUM>(&[m1a.deref(), m2a.deref()]).unwrap();
                let fib = g[0].wrapping_add(*g[1]);
                *g[1] = *g[0];
                *g[0] = fib;
            }
        }));
    }

    threads.into_iter().for_each(|t| {
        t.join().unwrap();
    });
}

// This is "persistent" algorithm from the https://howardhinnant.github.io/dining_philosophers.html
// However, it doesn't use any vector (even arrayvec), being simple and non-universal.
fn run_parallel_dumb<const THREAD_NUM: usize>(n: u64, swapped: bool) {
    let m1 = Arc::new(Mutex::new(1u64));
    let m2 = Arc::new(Mutex::new(1u64));
    let mut threads = Vec::with_capacity(THREAD_NUM);

    for i in 0..THREAD_NUM {
        let mut m1a = m1.clone();
        let mut m2a = m2.clone();
        threads.push(spawn(move || {
            if swapped && i & 1 == 0 {
                std::mem::swap(&mut m1a, &mut m2a);
            }
            for _ in 0..n {
                let mut g = loop {
                    let g1 = m1a.lock().unwrap();
                    match m2a.try_lock() {
                        Ok(g2) => break [g1, g2],
                        Err(_) => {
                            yield_now();
                            continue;
                        }
                    }
                };
                let fib = g[0].wrapping_add(*g[1]);
                *g[1] = *g[0];
                *g[0] = fib;
            }
        }));
    }

    threads.into_iter().for_each(|t| {
        t.join().unwrap();
    });
}

fn criterion_benchmark(c: &mut Criterion) {
    const COUNT: u64 = 20000;
    #[cfg(not(benchmark_swapped))]
    const SWAPPED: bool = false;
    #[cfg(benchmark_swapped)]
    const SWAPPED: bool = true;
    c.bench_function("run_parallel_lock_many", |b| {
        b.iter(|| run_parallel_lock_many::<2>(black_box(COUNT), SWAPPED))
    });
    c.bench_function("run_parallel_dumb", |b| {
        b.iter(|| run_parallel_dumb::<2>(black_box(COUNT), SWAPPED))
    });
    #[cfg(feature = "arrayvec")]
    c.bench_function("run_parallel_arrayvec", |b| {
        b.iter(|| run_parallel_lock_many_arrayvec::<2>(black_box(COUNT), SWAPPED))
    });

    c.bench_function("run_parallel_lock_many4", |b| {
        b.iter(|| run_parallel_lock_many::<4>(black_box(COUNT), SWAPPED))
    });
    c.bench_function("run_parallel_dumb4", |b| {
        b.iter(|| run_parallel_dumb::<4>(black_box(COUNT), SWAPPED))
    });
    #[cfg(feature = "arrayvec")]
    c.bench_function("run_parallel_arrayvec4", |b| {
        b.iter(|| run_parallel_lock_many_arrayvec::<4>(black_box(COUNT), SWAPPED))
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
