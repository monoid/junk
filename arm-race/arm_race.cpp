#include <atomic>
#include <cstdint>
#include <iostream>
#include <thread>

/* Improper locking (reproducible only with clang)

$ time ./a.out
1000000000 1000000000

real    1m19.583s
user    2m35.465s
sys     0m0.030s

 */
const auto LOCK_MEM_ORDER = std::memory_order_relaxed;
const auto UNLOCK_MEM_ORDER = std::memory_order_relaxed;

/* Proper locking:

$ time ./a.out                                                        
2000000000 2000000000                                                                                                                                                          
real    2m9.286s                                                                                                                                                               
user    3m56.303s                                                                                                                                                              
sys     0m0.011s                                                                                                                                                               

 */
// const auto LOCK_MEM_ORDER = std::memory_order_acquire;
// const auto UNLOCK_MEM_ORDER = std::memory_order_release;

using LockUnderlyingType = int;


struct Data {
  std::atomic<LockUnderlyingType> lock_;
  uint32_t var1 = 0;
  uint32_t var2 = 0;

  void increment(uint32_t count) {
    for (uint32_t i = 0; i < count; ++i) {
      LockUnderlyingType expected;
      do {
        expected = 0;
      } while (!lock_.compare_exchange_strong(expected, 1, LOCK_MEM_ORDER));
      uint32_t v1 = var1;
      uint32_t v2 = var2;
      var2 = v2 + 1;
      var1 = v1 + 1;
      lock_.store(0, UNLOCK_MEM_ORDER);
    }
  }
};


int main() {
  Data v;
  const auto count = 1000000000;
  auto t1 = std::thread([&v]() { v.increment(count); });
  auto t2 = std::thread([&v]() { v.increment(count); });
  t1.join();
  t2.join();
  
  std::cout << v.var1 << ' ' << v.var2 << std::endl;
}
