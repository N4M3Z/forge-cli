# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Test

```sh
make build              # cargo build --release
make install            # build, symlink to ~/.local/bin/forge, activate git hooks
make validate           # run pre-commit checks (prek → forge → validate.sh)
make test               # validate + cargo test
make clean              # remove build artifacts
```

Run a single test:

```sh
cargo test -- test_name
```

Pre-commit hook cascade: `prek run --all-files` → `forge validate .` → `scripts/validate.sh`. Activated by `make install` (sets `core.hooksPath` to `.githooks`). prek config in `.pre-commit-config.yaml`.

## Architecture

forge-cli is a two-stage content pipeline: **assemble** (transform source → `build/`) then **deploy** (`build/` → provider directories). The `install` command runs both stages.

### Pipeline Flow

```
source files → assemble (strip frontmatter, resolve variants, apply transforms) → build/{provider}/ → deploy → .claude/, .gemini/, .codex/, .opencode/
```

### Key Modules

| Module     | Path             | Purpose                                                       |
| ---------- | ---------------- | ------------------------------------------------------------- |
| `cli`      | `src/cli/`       | Clap subcommands — one directory per command with `mod.rs` + `tests.rs` |
| `assemble` | `src/assemble/`  | Strip frontmatter, resolve variant overrides, strip ref links |
| `transform`| `src/transform/` | Provider-specific transforms (kebab-case, tool remap, TOML)  |
| `validate` | `src/validate/`  | Module structure, `.mdschema` compliance, agent frontmatter   |
| `manifest` | `src/manifest/`  | `.manifest` read/write, SLSA provenance sidecars, staleness   |
| `provider` | `src/provider/`  | Provider config from `defaults.yaml` (targets, assembly rules) |
| `parse`    | `src/parse/`     | YAML frontmatter extraction (flat keys only, no nested YAML)  |
| `target`   | `src/target/`    | Deploy target resolution (scope, platform paths)              |
| `module`   | `src/module.rs`  | `module.yaml` deserialization                                 |
| `error`    | `src/error.rs`   | `ErrorKind` enum + `Error` struct                             |
| `result`   | `src/result.rs`  | `ActionResult` for structured command output                  |
| `yaml`     | `src/yaml/`      | YAML deep merge (defaults + config overlay)                   |

### Crate Structure

The `[lib]` crate is `commands` (exposed as `src/commands.rs`). The binary is `forge` (`src/main.rs` → `src/cli/`). Feature flags gate optional modules: `assemble`, `validate`, `deploy` (all on by default via `full`).

### Provider System

Provider conventions are config-driven via `defaults.yaml`. Each provider has a target directory, optional assembly rules, and optional deploy rules. Assembly rules are applied in order: `kebab-case`, `remap-tools`, `strip-links`, `agents-to-toml`.

Variant resolution uses qualifier directories (`user/`, `claude/`, `claude-opus-4/`) that flatten at assembly time. `user/` has highest precedence.

### Init Templates

`templates/init/` mirrors the deploy target 1:1 — no remapping config. `forge init <path>` iterates the directory and writes each file at the same relative path, substituting `${MODULE_NAME}`, `${VERSION}`, and `${VALIDATE_SH_SHA}` (the latter computed by `build.rs` from `scripts/validate.sh` and exposed as `commands::VALIDATE_SH_SHA`). Content `.mdschema` files live inside `templates/init/` at their deploy path (e.g. `agents/.mdschema`). Document schemas (README, CONTRIBUTING) live in `schemas/` — embedded for validation fallback only, never deployed.

### Consumer Manifest (`.forge`)

A non-module project that wants to use forge artifacts drops a `.forge` YAML file at its root listing the requested skills/agents/rules per producer source. `forge install --source <consumer-dir>` reads `.forge`, walks each declared source, filters its content to the requested subset, and runs the standard assemble + deploy pipeline scoped to the consumer's own provider directories. Parser, resolver, and git fetcher live at `src/cli/dotforge/` (`parse.rs`, `resolve.rs`, `filter.rs`, `git.rs`). `assemble::execute` branches on `.forge` presence to choose between `dotforge::resolve_sources` and the existing `sources::collect`.

Two source kinds:
- **Local** (`path: ../forge-core`) — sibling checkout on disk
- **Git** (`git: https://github.com/N4M3Z/forge-core`, `ref: <40-hex-SHA>`) — remote HTTPS repo pinned to a full commit SHA. Cloned via `gix` into `~/.cache/forge/git/<host>/<owner>/<repo>/` (override with `FORGE_GIT_CACHE_DIR`); the pinned tree is materialized into a per-SHA worktree dir. HTTPS-only, no shorthand or userinfo URLs, no branch / tag refs. `FORGE_GIT_ALLOW_FILE_URLS=1` allows `file://` URLs in tests.

### Validation

`forge validate` runs structural checks (module files, frontmatter, mdschema) plus manifest-based drift detection. If a `.manifest` exists, validate compares each tracked file's SHA-256 against the **current embedded template** — not the manifest fingerprint. The manifest indexes which files to check; the template is the source of truth for expected content. When forge-cli ships updated templates, validate catches modules that haven't updated.

Only files whose on-disk content matched the template at `forge init` time enter the manifest. Customized files (README, Makefile, defaults.yaml) stay out — no false DRIFT, no separate infrastructure/content lists.

External tool checks (shellcheck, cargo fmt/clippy, gitleaks, semgrep, ruff, tsc) run as fallback when prek is not installed. When prek is the orchestrator, `forge validate` skips external tools to avoid duplication.

Configurable excludes in `defaults.yaml` under `validate.exclude` — glob patterns for files to skip during YAML/JSON/whitespace checks (e.g. `templates/*` for template files with placeholders).

### Test Layout

Unit tests live as sibling `tests.rs` files next to `mod.rs` in every module. Integration tests in `tests/` with fixtures in `tests/fixtures/`. Fixtures loaded via `include_str!`.

## Conventions

- `Result<T, String>` for errors — no `anyhow`/`thiserror`
- `ErrorKind` enum for categorized errors (`Parse`, `Config`, `Io`, `Deploy`, `Validate`)
- `#[forbid(unsafe_code)]`, clippy pedantic enabled
- 4-space indentation everywhere
- All commands support `--json` for machine-readable output
- `defaults.yaml` (committed) + `config.yaml` (gitignored) deep merge pattern
- PRs required for all changes to `main` (branch ruleset enforced)
