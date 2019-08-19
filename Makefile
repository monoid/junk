CXX=g++
CXXFLAGS=-Wall -O2 -pthread -g -std=c++11

MALLOC_LIB=atomic_malloc.so
TEST_BIN=alloc_test

.PHONY: all test
all: $(TEST_BIN) $(MALLOC_LIB)

$(MALLOC_LIB):
	$(CXX) $(CXXFLAGS) -shared -fPIC -o $@ alloc.cpp malloc_wrapper.cpp

$(TEST_BIN): alloc_test.cpp
	$(CXX) $(CXXFLAGS) -o $@ alloc_test.cpp

test: all
	time ./$(TEST_BIN)
	LD_PRELOAD=./$(MALLOC_LIB) time ./$(TEST_BIN)
