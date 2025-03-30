#[unsafe(no_mangle)]
pub extern "C" fn alloc_buffer(size: i32) -> i32 {
    let data = vec![0u8; size as usize].into_boxed_slice();
    Box::<_>::into_raw(data) as *const u8 as usize as _
}

#[unsafe(no_mangle)]
pub extern "C" fn free_buffer(base: i32, size: i32) {
    let _ = unsafe {
        Box::<[u8]>::from_raw(std::slice::from_raw_parts_mut(
            base as usize as _,
            size as usize,
        ))
    };
}

#[unsafe(no_mangle)]
pub extern "C" fn hash(base: i32, size: i32) -> i32 {
    let data = unsafe {
        Box::<[u8]>::from_raw(std::slice::from_raw_parts_mut(
            base as usize as _,
            size as usize,
        ))
    };
    let hashed = blake3::hash(&data);
    let b: Box<[u8]> = hashed.as_bytes()[..].into();

    Box::<_>::into_raw(b) as *const u8 as usize as _
}
