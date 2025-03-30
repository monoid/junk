```
cargo build --release --target wasm32-unknown-unknown -p blake3_wasm_module
# or
emcc -msimd128 -O3 --no-entry -I$BLAKE_DIR/c \
   $BLAKE_DIR/c/{blake3.c,blake3_dispatch.c,blake3_portable.c,blake3_wasm32_simd.c} \
   wasm_module_c/main.c \
   -o wasm_module_c.wasm

cargo run --release -p blake3_wasm_executor ./target/wasm32-unknown-unknown/release/blake3_wasm_module.wasm
# or
cargo run --release -p blake3_wasm_executor ./wasm_module_c.wasm
```
