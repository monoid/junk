use dynasmrt::aarch64::Assembler;
use dynasmrt::{DynasmApi, ExecutableBuffer};

pub struct Function {
    _func: ExecutableBuffer,
    entry: unsafe extern "C" fn(a: i32, b: i32) -> i32,
}

impl Function {
    pub fn new() -> Function {
        let mut ops = Assembler::new().unwrap();

        let start = ops.offset();

        let frame_words = 4;
        generator::prologue(&mut ops, frame_words);
        generator::body(&mut ops);
        generator::epiloge(&mut ops, frame_words);

        let buf = ops.finalize().expect("finalize");
        let entry: unsafe extern "C" fn(a: i32, b: i32) -> i32 =
            unsafe { std::mem::transmute(buf.ptr(start)) };
        Function { _func: buf, entry }
    }

    pub fn call(&self, a: i32, b: i32) -> i32 {
        unsafe { (self.entry)(a, b) }
    }
}

mod generator {
    use dynasmrt::aarch64::Assembler;
    use dynasmrt::{dynasm, DynasmApi, DynasmLabelApi};

    pub(crate) fn prologue(ops: &mut Assembler, frame_words: usize) {
        let frame_bytes: u32 = (frame_words * 4).try_into().unwrap();

        dynasm!(ops
                ; .arch aarch64
                ; -> prologue:
                ; sub sp, sp, frame_bytes
        );
    }

    pub(crate) fn body(ops: &mut Assembler) {
        dynasm!(ops
                ; add w0, w0, w1
        );
    }

    pub(crate) fn epiloge(ops: &mut Assembler, frame_words: usize) {
        let frame_bytes: u32 = (frame_words * 4).try_into().unwrap();

        dynasm!(ops
                ; add sp, sp, frame_bytes
                ; ret
        );
    }
}
