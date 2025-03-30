```
cargo build --release --target wasm32-unknown-unknown -p blake3_wasm_module
cargo run --release -p blake3_wasm_executor ./target/wasm32-unknown-unknown/release/blake3_wasm_module.wasm
```
