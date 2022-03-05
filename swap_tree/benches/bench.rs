use std::ops::DerefMut;

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rotate_tree::{Node, rotate_in_place_deq, rotate_in_place_vec};

fn criterion_benchmark(c: &mut Criterion) {
    let mut cnt = 0;
    let mut tree12 = make_tree(18, &mut cnt);
    let mut tree16 = make_tree(24, &mut cnt);
    c.bench_function("deq 12", |b| b.iter(|| rotate_in_place_deq(black_box(tree12.deref_mut()))));
    c.bench_function("vec 12", |b| b.iter(|| rotate_in_place_vec(black_box(tree12.deref_mut()))));
    c.bench_function("deq 16", |b| b.iter(|| rotate_in_place_deq(black_box(tree16.deref_mut()))));
    c.bench_function("vec 16", |b| b.iter(|| rotate_in_place_vec(black_box(tree16.deref_mut()))));
}

fn make_tree(dep: usize, cnt: &mut u32) -> Box<Node<u32>> {
    Box::new(if dep == 0 {
        let n = Node::from(*cnt);
        *cnt += 1;
        n
    } else {
        let mut n = Node::from(*cnt);
        *cnt += 1;
        n.left = Some(make_tree(dep - 1, cnt));
        if dep > 1 {
            n.right = Some(make_tree(dep - 2, cnt));
        }
        n
    })
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
