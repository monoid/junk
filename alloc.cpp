#include "alloc.hpp"
#include <cassert>
#include <sys/mman.h>

constexpr std::size_t DEFAULT_ALLOC_SIZE = 64 * 1024;
constexpr size_t ALIGN_SIZE = 8;

static_assert(!(ALIGN_SIZE & (ALIGN_SIZE - 1)),
              "ALIGN_SIZE has to be a power of two");
static_assert(DEFAULT_ALLOC_SIZE % ALIGN_SIZE == 0,
              "DEFAULT_ALLOC_SIZE has to be aligned to ALIGN_SIZE'd");

thread_local char* free_end;
thread_local char* free_begin;


/**
 * Allocation size for sbrk.  sbrk is always called if requested
 * memory is larger than DEFAULT_ALLOC_SIZE, in this case we Returned
 * value is multiply of DEFAULT_ALLOC_SIZE.  Returned value is always
 * larger than requested, even if size is multiply of
 * DEFAULT_ALLOC_SIZE.
 */
static inline std::size_t SbrkAllocSize(std::size_t size) {
    size &= ~(DEFAULT_ALLOC_SIZE - 1);
    // Always sbrk more than requested.
    size += DEFAULT_ALLOC_SIZE;
    return size;
}


// Note that SbrkAllocSize(AlignSize(size)) == SbrkAllocSize(size)
static inline std::size_t AlignSize(std::size_t size) {
    // 8-bytes alignment
    if (size == 0) {
        return ALIGN_SIZE;
    }
    return (size + (ALIGN_SIZE - 1)) & ~(ALIGN_SIZE - 1);
}

char* MemorySingleton::AllocSbrk(std::size_t size) {
    // Name is misleading, we use mmap.
    // sbrk is not thread-safe!!!
    size_t allocSize = SbrkAllocSize(size);
    char* sbrk_new = static_cast<char*>(mmap(0, allocSize, PROT_READ|PROT_WRITE, MAP_PRIVATE|MAP_ANONYMOUS, -1, 0));
    assert((((intptr_t)sbrk_new) & (ALIGN_SIZE - 1)) == 0);
    free_begin = sbrk_new + size;
    free_end = sbrk_new + allocSize;

    return sbrk_new;
}

void* MemorySingleton::Allocate(std::size_t size) {
    size = AlignSize(size);
    char* start = free_begin;
    char* end = free_end;

    if (end && (start + size <= end)) {
      char* new_start = start + size;
      free_begin = new_start;
      return start;
    } else {
      return AllocSbrk(size);
    }
}
