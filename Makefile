CXX=clang++-9
CXXFLAGS=-Wall -O2 -pthread -g

.PHONY: all
all: alloc_test

alloc_test: alloc_test.cpp alloc.hpp alloc.cpp
	$(CXX) $(CXXFLAGS) -o $@ alloc_test.cpp alloc.cpp
