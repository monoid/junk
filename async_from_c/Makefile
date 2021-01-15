.PHONY: all run libexperiment clean

BIN=main

all: $(BIN)

main: main.c libexperiment 
	$(CC) -o $@ $< -L./target/release/ -lexperiment

libexperiment:
	cargo build --release

run: $(BIN)
	LD_LIBRARY_PATH=`pwd`/target/release ./$<

clean:
	rm -f $(BIN)
	cargo clean
