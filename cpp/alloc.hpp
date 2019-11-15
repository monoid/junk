#include <atomic>
#include <cstdint>

class MemorySingleton {
    static std::atomic<char*> free_end;
    static std::atomic<char*> free_begin;
    static std::atomic<bool> in_alloc;
    static std::atomic<std::size_t> alloc_stat;
    static std::atomic<std::size_t> sbrk_stat;

    static char* AllocSbrk(std::size_t size);
public:
    static void* Allocate(std::size_t size);
    static void PrintStats();
};
