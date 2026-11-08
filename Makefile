OBJS = $(shell find src -name '*.rs')

target/x86_64-unknown-linux-musl/release/seva: $(OBJS)
	PKG_CONFIG_SYS_ROOT_DIR=$(shell pwd)/pkgconfig PKG_CONFIG_PATH=$(shell pwd)/pkgconfig cargo build --release --target x86_64-unknown-linux-musl

install: target/x86_64-unknown-linux-musl/release/seva
	cp target/x86_64-unknown-linux-musl/release/seva /usr/bin/seva

clean:
	cargo clean