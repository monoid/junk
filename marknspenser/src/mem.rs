pub trait Mem {
    fn new(len: usize) -> Self;
    unsafe fn from_raw(base: *mut usize, len: usize) -> Self;
    fn to_raw(self) -> (*mut usize, usize);
}

impl Mem for Box<[usize]> {
    fn new(len: usize) -> Self {
        vec![0; len].into_boxed_slice()
    }

    unsafe fn from_raw(base: *mut usize, len: usize) -> Self {
        Box::from_raw(std::slice::from_raw_parts_mut(base, len))
    }

    fn to_raw(mut self) -> (*mut usize, usize) {
        let res = (self.as_mut_ptr(), self.len());
        std::mem::forget(self);
        res
    }
}
