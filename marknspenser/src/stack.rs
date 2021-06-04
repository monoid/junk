use std::{
    fs::File,
    io::{self, BufRead},
};

// C-string of /proc file that contains memory regions mapping info.
const SELF: &str = "/proc/self/maps";

pub fn find_range_in_mem_file(ptr: usize) -> Result<Option<(usize, usize)>, io::Error> {
    let mem_file = File::open(&SELF)?;
    for line in io::BufReader::new(mem_file).lines() {
        let line = line?;
        if let Some(range_str) = line.split_ascii_whitespace().next() {
            let mut sp = range_str.split('-');
            if let Some((first_str, second_str)) = sp.next().zip(sp.next()) {
                if let Ok(first) = usize::from_str_radix(first_str, 16) {
                    if let Ok(second) = usize::from_str_radix(second_str, 16) {
                        if (first <= ptr) & (ptr < second) {
                            return Ok(Some((first, second)));
                        }
                    }
                }
            }
        };
    }
    Ok(None)
}
