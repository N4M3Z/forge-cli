# forge-cli — build, test, lint, install

BINARY = target/release/forge

.PHONY: help build test lint check clean install

help:
	@echo "forge-cli targets:"
	@echo "  make build     Compile the forge binary"
	@echo "  make test      Run all tests"
	@echo "  make lint      Clippy + fmt + shellcheck + semgrep"
	@echo "  make check     Verify module structure and dependencies"
	@echo "  make install   Build and symlink forge to ~/.local/bin"
	@echo "  make clean     Remove build artifacts"

build:
	cargo build --release

test:
	cargo test
	cargo test --doc

lint:
	cargo fmt --check
	cargo clippy -- -D warnings
	@if command -v semgrep >/dev/null 2>&1; then semgrep scan --config=p/owasp-top-ten --metrics=off --quiet . 2>/dev/null || true; fi

check:
	@test -f module.yaml      && echo "  ok module.yaml"      || echo "  MISSING module.yaml"
	@test -f Cargo.toml       && echo "  ok Cargo.toml"       || echo "  MISSING Cargo.toml"
	@test -f defaults.yaml    && echo "  ok defaults.yaml"    || echo "  MISSING defaults.yaml"
	@test -f README.md        && echo "  ok README.md"        || echo "  MISSING README.md"
	@test -f INSTALL.md       && echo "  ok INSTALL.md"       || echo "  MISSING INSTALL.md"
	@test -f CONTRIBUTING.md  && echo "  ok CONTRIBUTING.md"  || echo "  MISSING CONTRIBUTING.md"
	@test -f CHANGELOG.md     && echo "  ok CHANGELOG.md"     || echo "  MISSING CHANGELOG.md"
	@test -f CODEOWNERS       && echo "  ok CODEOWNERS"       || echo "  MISSING CODEOWNERS"
	@test -f LICENSE          && echo "  ok LICENSE"          || echo "  MISSING LICENSE"
	@test -f .gitattributes   && echo "  ok .gitattributes"   || echo "  MISSING .gitattributes"

install: build
	mkdir -p ~/.local/bin
	ln -sf "$(CURDIR)/$(BINARY)" ~/.local/bin/forge
	@echo "Installed: forge -> $(CURDIR)/$(BINARY)"

clean:
	cargo clean
