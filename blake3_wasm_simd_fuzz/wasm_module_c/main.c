#include "emscripten.h"
#include "blake3.h"

int EMSCRIPTEN_KEEPALIVE alloc_buffer(int size) {
  return (int)malloc(size);
}

void EMSCRIPTEN_KEEPALIVE free_buffer(int base, int size) {
  free((void*) base);
}

int EMSCRIPTEN_KEEPALIVE hash(int base, int size) {
  uint8_t *result = malloc(32);

  blake3_hasher hasher;
  blake3_hasher_init(&hasher);
  blake3_hasher_update(&hasher, (uint8_t*)base, size);
  blake3_hasher_finalize(&hasher, result, 32);

  return (int)result;
}
