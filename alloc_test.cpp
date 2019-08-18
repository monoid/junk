#include "alloc.hpp"
#include <algorithm>
#include <iostream>
#include <vector>
#include <thread>
#include <utility>

struct List {
    List* next;
    void *payload;
    int value;
};

void AllocateNodes(int id, int count, List** result) {
    List* list = nullptr;
    for (int i = 0; i < count; ++i) {
        List* node = static_cast<List*>(MemorySingleton::Allocate(sizeof(List)));
        node->next = list;
        size_t payloadSize = 0;  // TODO
        node->payload = MemorySingleton::Allocate(payloadSize);
        list = node;
        node->value = id;
    }
    *result = list;
}

bool CheckList(const List* n, int id) {
    while (n) {
        if (n->value != id) {
            return false;
        }
        n = n->next;
    }
    return true;
}


void AddPointers(std::vector<std::pair<char*, char*>>* data, const List* list) {
    while (list) {
        char *l = (char*)list;
        char *p = (char*)(list->payload);
        data->emplace_back(std::make_pair(l, l + sizeof(List)));
        data->emplace_back(std::make_pair(p, p));
        list = list->next;
    }
}

void ValidatePointers(std::vector<std::pair<char*, char*>>* data) {
    std::sort(std::begin(*data), std::end(*data));
    for (auto it = std::next(std::begin(*data)); it < std::end(*data); ++it) {
        auto p = std::prev(it);
        if (p->second > it->first) {
            std::cerr << "FAILURE: " << std::endl
                      << (void*)(p->first) << " " << (void*)(p->second)  << std::endl
                      << (void*)(it->first) << " " << (void*)(it->second) << std::endl;
            break;
        }
    }
}

int main() {
    MemorySingleton::Init();
    List* n1;
    List* n2;
    List* n3;
    List* n4;

    void *a = MemorySingleton::Allocate(255);
    std::thread t1([&]() { AllocateNodes(1, 4000000, &n1); });
    std::thread t2([&]() { AllocateNodes(2, 4000000, &n2); });
    std::thread t3([&]() { AllocateNodes(3, 4000000, &n3); });
    std::thread t4([&]() { AllocateNodes(4, 4000000, &n4); });
    t1.join();
    t2.join();
    t3.join();
    t4.join();
    void *b = MemorySingleton::Allocate(100);

    std::cerr << a << ' ' << b << std::endl;
    std::cerr << CheckList(n1, 1) << " " << CheckList(n2, 2) << std::endl;
    std::cerr << CheckList(n3, 3) << " " << CheckList(n4, 4) << std::endl;

    MemorySingleton::PrintStat();

    std::vector<std::pair<char*, char*>> pointers;
    // Alot...
    pointers.reserve(2*4*4000000);
    AddPointers(&pointers, n1);
    AddPointers(&pointers, n2);
    AddPointers(&pointers, n3);
    AddPointers(&pointers, n4);
    ValidatePointers(&pointers);
    return 0;
}
