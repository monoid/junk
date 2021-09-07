.PHONY: test
CXXFLAGS=-std=c++11 -O3
CXX=clang++

test: main
	./main

main: arm_race.cpp
	$(CXX) $(CXXFLAGS) -o $@ $<
