#include <cstddef>
#include "alloc.hpp"

static void init() __attribute__((constructor));
static void finalize() __attribute__((destructor));

void init() {
   MemorySingleton::Init();
}

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

