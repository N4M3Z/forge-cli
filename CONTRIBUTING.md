# Contributing

## Getting Started

```sh
git clone https://github.com/N4M3Z/forge-cli.git
cd forge-cli
make build
make test
```

## Build & Test

See [README.md](README.md#build) for build targets. Run a single test:

```sh
cargo test -- test_name
```

## Architecture

Read `docs/decisions/` for the full ADR collection. Key decisions:

- **ASSEMBLY-0004** — two-stage pipeline (assemble → deploy) with intermediate build/
- **ASSEMBLY-0002** — SLSA/in-toto provenance sidecars
- **ASSEMBLY-0003** — deployment manifest (.manifest at target)
- **CLI-0006** — INSTALL.md following Mintlify standard

The README has a pipeline diagram.

## Code Conventions

- 4-space indentation everywhere
- No tab characters
- `cargo fmt` + `cargo clippy -- -D warnings` before committing
- `Result<T, String>` for error handling, no anyhow/thiserror
- `#[forbid(unsafe_code)]`
- Clippy pedantic enabled
- Fixtures in `tests/fixtures/`, loaded via `include_str!`
- Test files as siblings: `mod.rs` + `tests.rs`

## Git

Conventional Commits: `type: description`. Lowercase, no trailing period, no scope.

Types: `feat`, `fix`, `docs`, `chore`, `refactor`, `test`.

## Pull Requests

1. Fork and create a branch
2. Make changes following the conventions above
3. `make test`
4. Open a PR against `main`

CI runs `prek` (linting, formatting, tests) and `cargo test` on every PR. The `main` branch requires passing CI and one approval before merge.
