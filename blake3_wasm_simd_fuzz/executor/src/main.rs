use quickcheck::{QuickCheck, TestResult};

static mut cnt: usize = 0;

const MAX_TESTS: u64 = 10_000_000_000;
fn main() {
    let wasm = load();
    QuickCheck::new()
        .max_tests(MAX_TESTS)
        .tests(MAX_TESTS)
        .quickcheck(QcTest { wasm: wasm.into() });
    eprintln!("{}", unsafe { cnt });
}

struct QcTest {
    wasm: std::cell::RefCell<Blake3Wasm>,
}

impl quickcheck::Testable for QcTest {
    fn result(&self, g: &mut quickcheck::Gen) -> TestResult {
        unsafe { cnt += 1 };

        let data: Vec<u8> = quickcheck::Arbitrary::arbitrary(g);

        let expected = blake3::hash(&data);

        let mut wasm = self.wasm.borrow_mut();
        let result = exec(&mut *wasm, &data);

        let expected = expected.as_bytes();
        if expected == result.as_slice() {
            TestResult::passed()
        } else {
            TestResult::error(format!("Mismatch on {data:?}: {expected:?} != {result:?}"))
        }
    }
}

pub struct Blake3Wasm {
    engine: wasmtime::Engine,
    module: wasmtime::Module,
    linker: wasmtime::Linker<()>,
    store: wasmtime::Store<()>,
    instance: wasmtime::Instance,
    alloc: wasmtime::TypedFunc<i32, i32>,
    free: wasmtime::TypedFunc<(i32, i32), ()>,
    hash: wasmtime::TypedFunc<(i32, i32), i32>,
}

pub fn load() -> Blake3Wasm {
    let mut args = std::env::args_os();
    args.next();
    let path = args.next().expect("one argument with WASM binary path");
    let engine = wasmtime::Engine::default();
    let module = wasmtime::Module::from_file(&engine, path).expect("loading WASM");
    let mut linker = wasmtime::Linker::new(&engine);
    add_wasi_stubs(&mut linker);
    let mut store = wasmtime::Store::new(&engine, ());
    let instance = linker.instantiate(&mut store, &module).expect("instance");
    let alloc = instance
        .get_typed_func::<i32, i32>(&mut store, "alloc_buffer")
        .expect("alloc_buffer");
    let free = instance
        .get_typed_func::<(i32, i32), ()>(&mut store, "free_buffer")
        .expect("free_buffer");

    let hash = instance
        .get_typed_func::<(i32, i32), i32>(&mut store, "hash")
        .expect("blake3_hash function");

    Blake3Wasm {
        engine,
        module,
        linker,
        store,
        instance,
        alloc,
        free,
        hash,
    }
}

fn add_wasi_stubs(linker: &mut wasmtime::Linker<()>) {
    use wasmtime::Caller;

    const WASI_SNAPSHOT_PREVIEW1: &str = "wasi_snapshot_preview1";
    linker
        .func_wrap(
            WASI_SNAPSHOT_PREVIEW1,
            "fd_close",
            |_caller: Caller<'_, ()>, _a: i32| -1i32,
        )
        .unwrap();
    linker
        .func_wrap(
            WASI_SNAPSHOT_PREVIEW1,
            "fd_write",
            |_caller: Caller<'_, ()>, _a: i32, _b: i32, _c: i32, _d: i32| -1i32,
        )
        .unwrap();
    linker
        .func_wrap(
            WASI_SNAPSHOT_PREVIEW1,
            "fd_seek",
            |_caller: Caller<'_, ()>, _a: i32, _b: i64, _c: i32, _d: i32| -1i32,
        )
        .unwrap();
}

pub fn exec(wasm: &mut Blake3Wasm, data: &[u8]) -> Vec<u8> {
    let data_base = wasm
        .alloc
        .call(&mut wasm.store, data.len() as i32)
        .expect("alloc fail");
    let mem = wasm
        .instance
        .get_memory(&mut wasm.store, "memory")
        .expect("memory not found");
    mem.write(&mut wasm.store, data_base as usize, data)
        .expect("write memory");

    let result_base = wasm
        .hash
        .call(&mut wasm.store, (data_base, data.len() as i32))
        .expect("hash failed");
    let result_len = 32i32;

    let mut result = vec![0u8; result_len as usize];
    mem.read(&mut wasm.store, result_base as usize, &mut result[..])
        .expect("read failed");
    wasm.free
        .call(&mut wasm.store, (result_base, result_len))
        .expect("free failed");

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic() {
        let mut wasm = load();
        let hash = exec(&mut wasm, &[1, 23, 4, 5, 6, 78]);
        assert_eq!(
            hash,
            // from python impl
            &[
                150, 236, 29, 91, 212, 63, 110, 115, 212, 40, 148, 34, 246, 66, 101, 239, 197, 243,
                65, 139, 155, 102, 114, 36, 126, 85, 154, 77, 82, 108, 58, 231
            ],
        )
    }
}
