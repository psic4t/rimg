PREFIX ?= /usr/local
BIN = rimg
MAN = rimg.1
TARGET = target/release/$(BIN)

.PHONY: all build release clean install uninstall

all: build

build:
	cargo build

release:
ifdef SUDO_USER
	sudo -u $(SUDO_USER) cargo build --release
else
	cargo build --release
endif

install: release
	install -Dm755 $(TARGET) $(DESTDIR)$(PREFIX)/bin/$(BIN)
	install -Dm644 $(MAN) $(DESTDIR)$(PREFIX)/share/man/man1/$(MAN)
	install -Dm644 rimg.desktop $(DESTDIR)$(PREFIX)/share/applications/rimg.desktop
	install -Dm644 README.md $(DESTDIR)$(PREFIX)/share/doc/$(BIN)/README.md

uninstall:
	rm -f $(DESTDIR)$(PREFIX)/bin/$(BIN)
	rm -f $(DESTDIR)$(PREFIX)/share/man/man1/$(MAN)
	rm -f $(DESTDIR)$(PREFIX)/share/applications/rimg.desktop
	rm -rf $(DESTDIR)$(PREFIX)/share/doc/$(BIN)

clean:
	cargo clean

check:
	cargo check

fmt:
	cargo fmt

clippy:
	cargo clippy -- -D warnings
