#[derive(Copy, Clone, Debug)]
pub enum Mode {
    Crc64,
    CityHash64,
    Other,
}

#[inline]
fn crc64f(s: &str) -> u64 {
    crc64::crc64(0, s.as_bytes())
}

#[inline]
fn cityhash64f(s: &str) -> u64 {
    cityhash_sys::city_hash_64(s.as_bytes())
}

#[inline]
fn otherf(_s: &str) -> u64 {
    1
}

pub fn hash_e(m: Mode, s: &[&str]) -> u64 {
    let mut v = 0;
    for e in s {
        v += match m {
            Mode::Crc64 => crc64f(e),
            Mode::CityHash64 => cityhash64f(e),
            Mode::Other => otherf(e),
        }
    }
    v
}

pub fn hash_i(m: Mode, s: &[&str]) -> u64 {
    let mut v = 0;
    let f = match m {
        Mode::Crc64 => crc64f,
        Mode::CityHash64 => cityhash64f,
        Mode::Other => otherf,
    };
    for e in s {
        v += f(e);
    }
    v
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
