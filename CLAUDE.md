# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Test

```sh
make build              # cargo build --release
make test               # cargo test + doc tests
make lint               # cargo fmt --check + clippy + semgrep OWASP
make check              # verify module structure files exist
make install            # build + symlink to ~/.local/bin/forge + configure pre-commit hook
```

Run a single test:

```sh
cargo test -- test_name
```

Pre-commit hook runs `scripts/validate.sh` via `.githooks/pre-commit`. Activated by `make install` (sets `core.hooksPath`).

## Architecture

forge-cli is a two-stage content pipeline: **assemble** (transform source → `build/`) then **deploy** (`build/` → provider directories). The `install` command runs both stages.

### Pipeline Flow

```
source files → assemble (strip frontmatter, resolve variants, apply transforms) → build/{provider}/ → deploy → .claude/, .gemini/, .codex/, .opencode/
```

### Key Modules

| Module          | Path                | Purpose                                                      |
| --------------- | ------------------- | ------------------------------------------------------------ |
| `cli`           | `src/cli/`          | Clap subcommands — one file per command                      |
| `assemble`      | `src/assemble/`     | Strip frontmatter, resolve variant overrides, strip ref links |
| `transform`     | `src/transform/`    | Provider-specific transforms (kebab-case, tool remap, TOML)  |
| `validate`      | `src/validate/`     | Module structure, `.mdschema` compliance, agent frontmatter  |
| `manifest`      | `src/manifest/`     | `.manifest` read/write, SLSA provenance sidecars, staleness  |
| `provider`      | `src/provider/`     | Provider config from `defaults.yaml` (targets, assembly rules) |
| `parse`         | `src/parse/`        | YAML frontmatter extraction from markdown                    |
| `target`        | `src/target/`       | Deploy target resolution (scope, platform paths)             |
| `module`        | `src/module.rs`     | `module.yaml` deserialization                                |
| `error`         | `src/error.rs`      | `ErrorKind` enum + `Error` struct                            |
| `result`        | `src/result.rs`     | `ActionResult` for structured command output                 |
| `yaml`          | `src/yaml/`         | YAML deep merge (defaults + config overlay)                  |

### Crate Structure

The `[lib]` crate is `commands` (exposed as `src/commands.rs`). The binary is `forge` (`src/main.rs` → `src/cli/`). Feature flags gate optional modules: `assemble`, `validate`, `deploy` (all on by default via `full`).

### Provider System

Provider conventions are config-driven via `defaults.yaml`. Each provider has a target directory, optional assembly rules, and optional deploy rules. Assembly rules are applied in order: `kebab-case`, `remap-tools`, `strip-links`, `agents-to-toml`.

Variant resolution uses qualifier directories (`user/`, `claude/`, `claude-opus-4/`) that flatten at assembly time. `user/` has highest precedence.

### Test Layout

Unit tests live as sibling `tests.rs` files next to `mod.rs`. Integration tests in `tests/` with fixtures in `tests/fixtures/`. Fixtures loaded via `include_str!`.

## Conventions

- `Result<T, String>` for errors — no `anyhow`/`thiserror`
- `ErrorKind` enum for categorized errors (`Parse`, `Config`, `Io`, `Deploy`, `Validate`)
- `#[forbid(unsafe_code)]`, clippy pedantic enabled
- 4-space indentation everywhere
- All commands support `--json` for machine-readable output
- `defaults.yaml` (committed) + `config.yaml` (gitignored) deep merge pattern
