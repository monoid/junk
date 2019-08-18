CXX=clang++-9
CXXFLAGS=-Wall -O2 -pthread -g -std=c++11 $(EXTRA_CXXFLAGS)

.PHONY: all
all: alloc_test

alloc_test: alloc_test.cpp alloc.hpp alloc.cpp
	$(CXX) $(CXXFLAGS) -o $@ alloc_test.cpp alloc.cpp
