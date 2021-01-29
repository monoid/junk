use std::ffi::{CStr, CString};
use std::os::raw::c_char;

use tokio_stream::StreamExt;

// "On Unix systems when pthread-based TLS is being used, destructors
// will not be run for TLS values on the main thread when it
// exits. Note that the application will exit immediately after the
// main thread exits as well." (std::thread::LocalKey).
thread_local! {
    static RUNTIME: tokio::runtime::Runtime =tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
}

#[no_mangle]
extern "C" fn query(url: *const c_char) -> *mut c_char {
    match unsafe { CStr::from_ptr(url).to_str() } {
        Ok(url) => {
            let data = RUNTIME.with(|r| {
                r.block_on(async {
                    let mut data = vec![0u8; 0];
                    match reqwest::get(url).await {
                        Ok(res) => {
                            let mut stream = res.bytes_stream();
                            while let Some(chunk) = stream.next().await {
                                match chunk {
                                    Ok(chunk) => {
                                        data.extend_from_slice(&chunk[..]);
                                    }
                                    Err(_) => {
                                        return None;
                                    }
                                }
                            }
                        }
                        Err(_) => {
                            return None;
                        }
                    }
                    Some(data)
                })
            });
            data.and_then(|d| CString::new(d).ok())
                .map(CString::into_raw)
                .unwrap_or(std::ptr::null_mut())
        }
        Err(_) => std::ptr::null_mut(),
    }
}

#[no_mangle]
extern "C" fn free_result(data: *mut c_char) {
    if !data.is_null() {
        unsafe { CString::from_raw(data) };
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
