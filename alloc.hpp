#include <cstdint>

class MemorySingleton {
    static char* AllocSbrk(std::size_t size);
public:
    static void Init() { }
    static void* Allocate(std::size_t size);
    static void PrintStats() { }
};
