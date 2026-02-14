PREFIX ?= /usr/local
BIN = rimg
MAN = rimg.1
SRC = $(shell find src -name '*.rs')
TARGET = target/x86_64-unknown-linux-gnu/release/$(BIN)

ifdef SUDO_USER
  RUSTUP_HOME ?= $(shell echo ~$(SUDO_USER))/.rustup
else
  RUSTUP_HOME ?= $(HOME)/.rustup
endif
export RUSTUP_HOME

.PHONY: all build release clean install uninstall

all: build

build:
	cargo build

release:
	cargo build --release

$(TARGET): $(SRC)
	cargo build --release

install: $(TARGET)
	install -Dm755 $(TARGET) $(DESTDIR)$(PREFIX)/bin/$(BIN)
	install -Dm644 $(MAN) $(DESTDIR)$(PREFIX)/share/man/man1/$(MAN)
	install -Dm644 README.md $(DESTDIR)$(PREFIX)/share/doc/$(BIN)/README.md

uninstall:
	rm -f $(DESTDIR)$(PREFIX)/bin/$(BIN)
	rm -f $(DESTDIR)$(PREFIX)/share/man/man1/$(MAN)
	rm -rf $(DESTDIR)$(PREFIX)/share/doc/$(BIN)

clean:
	cargo clean

check:
	cargo check

fmt:
	cargo fmt

clippy:
	cargo clippy -- -D warnings
