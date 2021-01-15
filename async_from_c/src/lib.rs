use std::os::raw::c_char;
use std::ffi::{CStr, CString};

use once_cell::sync::Lazy;
use tokio_stream::StreamExt;

static RUNTIME: Lazy<tokio::runtime::Runtime> = Lazy::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
});

#[no_mangle]
extern "C"
fn query(url: *const c_char) -> *mut c_char {
    match unsafe { CStr::from_ptr(url).to_str() } {
        Ok(url) => {
            let data = RUNTIME.block_on(async {
                let mut data = vec![0u8; 0];
                match reqwest::get(url).await {
                    Ok(res) => {
                        let mut stream = res.bytes_stream();
                        while let Some(chunk) = stream.next().await {
                            match chunk {
                                Ok(chunk) => {
                                    data.extend_from_slice(&chunk[..]);
                                },
                                Err(_) => {
                                    return None;
                                }
                            }
                        }
                    },
                    Err(_) => {
                        return None;
                    }
                }
                Some(data)
            });
            data.and_then(
                |d| CString::new(d).ok()
            ).map(
                CString::into_raw
            ).unwrap_or(
                std::ptr::null_mut()
            )
        },
        Err(_) => {
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
extern "C" fn free_result(data: *mut c_char) {
    unsafe {
        CString::from_raw(data)
    };
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
