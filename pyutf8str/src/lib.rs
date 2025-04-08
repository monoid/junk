use std::ffi::c_void;

use pyo3::ffi::_Py_HashBytes;
use pyo3::prelude::*;
use pyo3::pyclass::CompareOp;
use pyo3::types::PyString;

#[pyclass]
struct Utf8Str {
    val: String,
    char_len: usize,
    hash: isize,
}

#[pymethods]
impl Utf8Str {
    // PyO3 kindly transforms the python str into Rust String.  We do not need
    // anything but to use it.
    #[new]
    fn new(val: String) -> Self {
        let mut max_char = '\x7F';
        let mut char_len = 0;
        for c in val.chars() {
            max_char = std::cmp::max(c, max_char);
            char_len += 1;
        }
        let hash = Self::py_hash(&val, max_char, char_len);
        Self {
            val,
            char_len,
            hash,
        }
    }

    // Againt, as we simply returns a reference, PyO3 does the conversion for us.
    #[inline]
    fn __str__(&self) -> &str {
        &self.val
    }

    #[inline]
    fn __bytes__(&self) -> &[u8] {
        self.val.as_bytes()
    }

    #[inline]
    fn __len__(&self) -> usize {
        self.char_len
    }

    fn __repr__(&self) -> String {
        let mut res = String::new();
        res.push('\'');
        // It can be made faster with some unsafe code:
        // one should look for the special chars,
        // copying all intermediate bytes as is.
        // It differs from Python implementation, consider it as a STUB.
        // For example, it does not escape non-printable characters.
        //
        // We also might use PyUnicode methods.
        for c in self.val.chars() {
            match c {
                '\'' | '\"' | '\\' => {
                    res.push('\\');
                    res.push(c);
                }
                other => res.push(other),
            }
        }
        res.push('\'');
        res
    }

    // Seems to be compatible with Unicode chars.
    #[inline]
    fn __richcmp__(&self, other: &Bound<'_, PyAny>, op: CompareOp) -> PyResult<bool> {
        if let Ok(guard) = other.downcast::<Utf8Str>() {
            return Ok(self.richcmp(guard.borrow().val.as_str(), op));
        } else if let Ok(uni) = other.downcast::<PyString>() {
            return Ok(self.richcmp(uni.to_str()?, op));
        }
        Ok(matches!(op, CompareOp::Ne))
    }

    #[inline]
    fn __hash__(&self) -> isize {
        self.hash
    }

    // Well, __len__ is enough, but we still provide the __bool__.
    #[inline]
    fn __bool__(&self) -> bool {
        !self.val.is_empty()
    }
}

impl Utf8Str {
    // Helper method.
    #[inline]
    fn richcmp(&self, other: &str, op: CompareOp) -> bool {
        let val = self.val.as_str();
        match op {
            CompareOp::Lt => val < other,
            CompareOp::Le => val <= other,
            CompareOp::Eq => val == other,
            CompareOp::Ne => val != other,
            CompareOp::Gt => val > other,
            CompareOp::Ge => val >= other,
        }
    }

    fn py_hash(val: &str, max_char: char, char_len: usize) -> isize {
        if max_char <= '\x7F' {
            unsafe { _Py_HashBytes(val.as_bytes().as_ptr() as *const c_void, val.len() as isize) }
        } else if max_char <= '\u{FF}' {
            let mut vec = Vec::with_capacity(char_len);
            for c in val.chars() {
                vec.push(c as u8);
            }
            unsafe {
                _Py_HashBytes(
                    vec.as_ptr() as _,
                    (char_len * std::mem::size_of::<u8>()) as _,
                )
            }
        } else if max_char <= '\u{FFFF}' {
            let mut vec = Vec::with_capacity(char_len);
            for c in val.chars() {
                vec.push(c as u16);
            }
            unsafe {
                _Py_HashBytes(
                    vec.as_ptr() as _,
                    (char_len * std::mem::size_of::<u16>()) as _,
                )
            }
        } else {
            let mut vec = Vec::with_capacity(char_len);
            for c in val.chars() {
                vec.push(c as u32);
            }
            unsafe {
                _Py_HashBytes(
                    vec.as_ptr() as _,
                    (char_len * std::mem::size_of::<u32>()) as _,
                )
            }
        }
    }
}

#[pymodule]
pub fn pyutf8str(_py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Utf8Str>()?;
    Ok(())
}
