#include "alloc.hpp"
#include <cassert>
#include <stdexcept>
#include <iostream>
#include <iomanip>
#include <sys/mman.h>

constexpr std::size_t DEFAULT_ALLOC_SIZE = 64 * 1024;
constexpr size_t ALIGN_SIZE = 8;

static_assert(!(ALIGN_SIZE & (ALIGN_SIZE - 1)),
              "ALIGN_SIZE has to be a power of two");
static_assert(DEFAULT_ALLOC_SIZE % ALIGN_SIZE == 0,
              "DEFAULT_ALLOC_SIZE has to be aligned to ALIGN_SIZE'd");

std::atomic<char*> MemorySingleton::free_end{0};
std::atomic<char*> MemorySingleton::free_begin{0};
std::atomic<bool> MemorySingleton::in_alloc{0};
std::atomic<std::size_t> MemorySingleton::alloc_stat{0};
std::atomic<std::size_t> MemorySingleton::sbrk_stat{0};

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
    bool in_alloc_expected = false;
    size_t allocSize = SbrkAllocSize(size);
    //std::cerr << "Sbrk size " << allocSize << " for " << size << std::endl;
    if (in_alloc.compare_exchange_weak(in_alloc_expected, true)) {
        char* sbrk_new = static_cast<char*>(mmap(0, allocSize, PROT_READ|PROT_WRITE, MAP_PRIVATE|MAP_ANONYMOUS, -1, 0));
        sbrk_stat.fetch_add(allocSize);
        if (sbrk_new == reinterpret_cast<void*>(-1)) {
            throw std::runtime_error("OOM");
        } else {
            assert((((intptr_t)sbrk_new) & (ALIGN_SIZE - 1)) == 0);
            // We have to update both begin and end together!!!
            free_begin.store(sbrk_new + size);
            // Now free_end < free_begin, no allocation in other
            // thread can happen.
            
            // Updating end.
            free_end.store(sbrk_new + allocSize);
            in_alloc.store(false);
            return sbrk_new;
        }
    } else {
        return nullptr;
    }
}

void* MemorySingleton::Allocate(std::size_t size) {
    size = AlignSize(size);
    while (true) {
        char* end = free_end.load();
        // Order of fetching end and start is important 8-)>
        // If AllocSbrk will happen before fetching end and start,
        // end < start, and the loop will just restart immediately.
        // If you change order of these statements, it will be
        // start < end, but start and end will be from different
        // memory regions...
        char* start = free_begin.load();

        // It REALLY affects sbrk size!
        if (end && start > end) {
            continue;
        }
            
        //std::cerr << (void*)start << " -> " << (void*)end << std::endl;
        if (end && (start + size <= end)) {
            /* ^ We check here for "end" because it is
             * initialized/updated last */

            char* new_start = start + size;
            if (free_begin.compare_exchange_weak(start, new_start)) {
                alloc_stat.fetch_add(size);
                return start;
            }
            // else continue;
        } else {
            //std::cerr << "Try sbrk" << std::endl;
            start = AllocSbrk(size);
            
            if (start) {
                alloc_stat.fetch_add(size);
                return start;
            }
            // else continue;
        }
    }
}


void MemorySingleton::PrintStats() {
    std::cerr << "sbrk size:  " << std::setw(18) << sbrk_stat.load() << std::endl
              << "alloc size: " << std::setw(18) << alloc_stat.load() << std::endl
              << "now free:   " << std::setw(18) << free_end.load() - free_begin.load() << std::endl;
}
