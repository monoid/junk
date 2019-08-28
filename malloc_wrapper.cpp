#include <cstddef>
#include "alloc.hpp"

static void finalize() __attribute__((destructor));

void finalize() {
   MemorySingleton::PrintStats();
}

extern "C" 
void* malloc(size_t sz) {
   return MemorySingleton::Allocate(sz);
}

extern "C" 
void free(void*) {
}

