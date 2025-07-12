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
    use armenia::instructions::arith::add::add;
    use armenia::instructions::arith::sub::sub;
    use armenia::instructions::branches::ret;
    use armenia::instructions::Instruction as _;
    use armenia::register::Reg32::*;
    use armenia::register::RegOrSp64::SP;
    use dynasmrt::aarch64::Assembler;

    pub(crate) fn prologue(ops: &mut Assembler, frame_words: usize) {
        let frame_bytes: u32 = (frame_words * 4).try_into().unwrap();

        ops.extend(sub(SP, SP, frame_bytes).unwrap().bytes());
    }

    pub(crate) fn body(ops: &mut Assembler) {
        ops.extend(add(W0, W0, W1).unwrap().bytes());
    }

    pub(crate) fn epiloge(ops: &mut Assembler, frame_words: usize) {
        let frame_bytes: u32 = (frame_words * 4).try_into().unwrap();

        ops.extend(add(SP, SP, frame_bytes).unwrap().bytes());
        ops.extend(ret().represent().flat_map(|x| x.0));
    }
}
