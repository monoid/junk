use quickcheck::{QuickCheck, TestResult};

pub const WASM_BIN_PATH_ENV: &str = "WASM_BIN_PATH";

static mut cnt: usize = 0;

const MAX_TESTS: u64 = 100_000_000_000;
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

        TestResult::from_bool(&expected.as_bytes()[..] == result.as_slice())
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
    let path = std::env::var(WASM_BIN_PATH_ENV).expect("WASM_BIN_PATH variable");
    let engine = wasmtime::Engine::default();
    let module = wasmtime::Module::from_file(&engine, path).expect("loading WASM");
    let linker = wasmtime::Linker::new(&engine);
    let mut store = wasmtime::Store::new(&engine, ());
    let instance = linker.instantiate(&mut store, &module).expect("instance");
    let alloc = instance
        .get_typed_func::<i32, i32>(&mut store, "alloc")
        .expect("alloc");
    let free = instance
        .get_typed_func::<(i32, i32), ()>(&mut store, "free")
        .expect("free");

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
