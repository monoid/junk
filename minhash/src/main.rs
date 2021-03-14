use itertools::Itertools as _;
use std::env;
use std::fs::File;
use std::io::{Read as _, BufReader};
use fasthash::{city::Hash32, FastHash};

struct State {
    max_size: usize,
    // heap is not very effective for small size, but saves my time.
    heap: std::collections::BinaryHeap<u32>,
}

impl State {
    fn new() -> Self {
        let max_size = 5;
        let mut heap = std::collections::BinaryHeap::with_capacity(max_size + 1);
        // Prefill with max values for short files
        for _ in 0..max_size {
            heap.push(-1 as _);
        }
        Self {
            max_size,
            heap,
        }
    }

    fn push(&mut self, val: u32) {
        self.heap.push(val);
        if self.heap.len() > self.max_size {
            self.heap.pop();
        }
    }
}

impl std::fmt::Display for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for val in self.heap.clone().into_sorted_vec() {
            write!(f, "{:08x}", val)?;
        }
        Ok(())
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    for path in env::args().skip(1) {
        let inp = BufReader::new(File::open(&path)?);
        let mut state = State::new();
        for (a, b, c) in inp.bytes().flat_map(|r| r.ok()).tuple_windows() {
            let hash = Hash32::hash_with_seed(&[a, b, c], 1);
            state.push(hash);
        }
        println!("{}\t{}", state, path);
    }
    Ok(())
}
