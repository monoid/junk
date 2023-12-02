use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rand::thread_rng;
use rand::seq::SliceRandom;
use c3_5_onepass::*;


fn criterion_benchmark(c: &mut Criterion) {
    let mut data: Vec<u64> = (0..(1 << 8)).collect();
    data.shuffle(&mut thread_rng());

    c.bench_function("popcnt", |b| b.iter(|| {
        for val in black_box(&data).iter().copied() {
            black_box(popcnt_count(val));
        }
    }));
    c.bench_function("table", |b| b.iter(|| {
        for val in black_box(&data).iter().copied() {
            black_box(table_count(val));
        }
    }));
}

fn criterion_benchmark_single(c: &mut Criterion) {
    c.bench_function("popcnt", |b| b.iter(|| {
            popcnt_count(black_box(759642677176073960));
    }));
    c.bench_function("table", |b| b.iter(|| {
            table_count(black_box(759642677176073960));
    }));
}

criterion_group!(benches, criterion_benchmark, criterion_benchmark_single);
criterion_main!(benches);
