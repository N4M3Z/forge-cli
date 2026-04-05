# forge-cli

FORGE ?= forge
BINARY = target/release/forge

.PHONY: help build install validate test clean

help:
	@echo "  make build      compile the forge binary"
	@echo "  make install    build, symlink, activate git hooks"
	@echo "  make validate   run pre-commit checks"
	@echo "  make test       validate + cargo test"
	@echo "  make clean      remove build artifacts"

build:
	cargo build --release

install: build
	mkdir -p ~/.local/bin
	ln -sf "$(CURDIR)/$(BINARY)" ~/.local/bin/forge
	git config core.hooksPath .githooks
	@echo "Installed: forge -> $(CURDIR)/$(BINARY)"

validate:
	@bash .githooks/pre-commit

test: validate
	cargo test

clean:
	cargo clean
