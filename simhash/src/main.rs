use fasthash::{city::Hash32, FastHash};
use itertools::Itertools as _;
use std::fs::File;
use std::io::{BufReader, Read as _};
use std::env;

struct State {
    counts: [isize; 8 * std::mem::size_of::<u32>()],
}

impl State {
    fn new() -> Self {
        Self {
            counts: Default::default()
        }
    }

    fn push(&mut self, val: u32) {
        for (place, bit) in self.counts.iter_mut().zip((0u32..).map(|i| (val >> i) & 1)) {
            if bit != 0 {
                *place += 1;
            } else {
                *place -= 1;
            }
        }
    }
}

impl std::fmt::Display for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let val = self.counts
            .iter()
            .copied()
            .map(|c| if c > 0 { 1u32 } else { 0 })
            .zip(0..)
            .map(|(flag, off)| flag << off)
            .fold(0, |a, b| a | b);
        write!(f, "{:08x}", val)
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
