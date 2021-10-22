use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
use bench_indirect::*;

criterion_group!(benches, bench_compare);
criterion_main!(benches);

fn bench_compare(c: &mut Criterion) {
    let mut group = c.benchmark_group("hashing");
    let DATA = &["test", "me", "one", "more time, please"][..];
    group.bench_with_input(BenchmarkId::new("direct", "crc64"), &(Mode::Crc64, DATA),
                           |b, i| b.iter(|| hash_e(i.0, i.1)));
    group.bench_with_input(BenchmarkId::new("indirect", "crc64"), &(Mode::Crc64, DATA), 
                           |b, i| b.iter(|| hash_i(i.0, i.1)));
    group.bench_with_input(BenchmarkId::new("direct", "cityhash64"), &(Mode::CityHash64, DATA),
                           |b, i| b.iter(|| hash_e(i.0, i.1)));
    group.bench_with_input(BenchmarkId::new("indirect", "cityhash64"), &(Mode::CityHash64, DATA), 
                           |b, i| b.iter(|| hash_i(i.0, i.1)));
    group.finish();
}
