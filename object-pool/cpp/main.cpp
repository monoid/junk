/** Translation of Rust pool into C++ */

#include <vector>
#include <memory>

template<class Intern, class Interned>
class DumbSet {
private:
  std::vector<std::weak_ptr<Interned>> bins;
public:
  DumbSet() : bins{} {
  }
  
  std::shared_ptr<Interned> intern(const Intern& val) {
    for (const auto& weak: bins) {
      std::shared_ptr<Interned> strong = weak.lock();
      // TODO: also check hash before the `==`
      if (strong && *strong == val) {
        return strong;
      }
    }
    // Not found: insert new element.
    std::shared_ptr<Interned> res = std::make_shared<Interned>(Interned{val});
    // TODO: insert at free place, if any
    bins.push_back(std::weak_ptr<Interned>{res});
    return res;
  }
  
  void implant(const std::shared_ptr<Interned>& val) {
    for (const auto& weak: bins) {
      std::shared_ptr<Interned> strong = weak.lock();
      // TODO: also check hash before the `==`
      if (strong && *strong == *val) {
        return;
      }
    }
    // TODO: insert at free place, if any
    bins.push_back(std::weak_ptr<Interned>{val});
  }
};

#include <string>
#include <iostream>
int main() {
  DumbSet<const char*, std::string> set;
  auto v1 = set.intern("test");
  auto v2 = set.intern("test");
  auto v3 = set.intern("test2");

  std::cout << (v1.get() == v2.get()) << std::endl
	    << (v1.get() == v3.get()) << std::endl;
  return 0;
}
