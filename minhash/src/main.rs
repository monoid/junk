use cityhasher::hash;
use sliding_windows::{IterExt, Storage};
use std::fs::File;
use std::io::{BufReader, Read as _};
use std::{cmp::Reverse, collections::BinaryHeap, env};

struct State {
    max_size: usize,
    // heap is not very effective for small size, but saves my time.
    heap_min: BinaryHeap<u32>,
    heap_max: BinaryHeap<Reverse<u32>>,
}

impl State {
    fn new() -> Self {
        let max_size = 5;
        let mut heap_min = BinaryHeap::with_capacity(max_size + 1);
        let mut heap_max = BinaryHeap::with_capacity(max_size + 1);
        // Prefill with max values for short files
        for _ in 0..max_size {
            heap_min.push(-1 as _);
            heap_max.push(Reverse(0));
        }
        Self {
            max_size,
            heap_min,
            heap_max,
        }
    }

    fn push(&mut self, val: u32) {
        if !self.heap_min.iter().any(|v| *v == val) {
            self.heap_min.push(val);
            if self.heap_min.len() > self.max_size {
                self.heap_min.pop();
            }
        }

        if !self.heap_max.iter().any(|v| v.0 == val) {
            self.heap_max.push(Reverse(val));
            if self.heap_max.len() > self.max_size {
                self.heap_max.pop();
            }
        }
    }
}

impl std::fmt::Display for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for val in self.heap_min.clone().into_sorted_vec() {
            write!(f, "{:08x},", val)?;
        }
        write!(f, "|")?;
        for val in self.heap_max.clone().into_sorted_vec() {
            write!(f, "{:08x},", val.0)?;
        }
        Ok(())
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    const WINDOW: usize = 8;

    for path in env::args().skip(1) {
        let inp = BufReader::new(File::open(&path)?);
        let mut state = State::new();
        let mut storage = Storage::new(WINDOW);
        for window in inp
            .bytes()
            .flat_map(|r| r.ok())
            .sliding_windows(&mut storage)
        {
            let hash = hash(
                window
                    .into_iter()
                    .copied()
                    .collect::<arrayvec::ArrayVec<[u8; WINDOW]>>(),
            );
            state.push(hash);
        }
        println!("{}\t{}", state, path);
    }
    Ok(())
}
