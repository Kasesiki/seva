OBJS = $(shell find src -name '*.rs')

target/release/seva: $(OBJS)
	cargo build --release

install: target/release/seva
	cp target/release/seva /usr/bin/seva

clean:
	cargo clean