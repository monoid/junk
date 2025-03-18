```
cargo build --release --target wasm32-unknown-unknown -p blake3_wasm_module
export WASM_BIN_PATH=$PWD/target/wasm32-unknown-unknown/release/blake3_wasm_module.wasm
cargo run --release -p blake3_wasm_executor
```
