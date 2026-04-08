# GEMINI.md - forge-cli

This file provides a comprehensive overview of the `forge-cli` project, its architecture, and development conventions for the Gemini CLI agent.

## Project Overview

`forge-cli` is a Rust-based toolkit designed to **assemble, validate, and deploy** markdown-based content (skills, agents, and rules) across multiple AI coding providers (Claude, Gemini, Codex, OpenCode).

The core philosophy is "Author Once, Deploy Everywhere." Authors write provider-agnostic markdown with YAML frontmatter, and `forge-cli` handles the provider-specific transformations, path flattening, and provenance tracking.

### Key Features
- **Two-Stage Pipeline:** Separates content transformation (**assemble**) from file placement (**deploy**).
- **Variant Resolution:** Supports overrides via `user/`, `provider/`, and `model/` subdirectories with a clear precedence order (`user/` > `provider/model/` > `provider/` > root).
- **Provenance & Manifests:** Generates SLSA/in-toto sidecars and `.manifest` files to track source-to-deployed chains and detect local modifications.
- **Validation:** Enforces module structure and `.mdschema` compliance for agents and documents.
- **Provider-Specific Transforms:** Handles `kebab-case` filenames, tool name remapping, and TOML conversion (for Codex).

### Technologies
- **Language:** Rust (2024 edition)
- **CLI Framework:** `clap`
- **Serialization:** `serde`, `serde_yaml`, `serde_json`
- **Utils:** `regex`, `chrono`, `sha2`, `rust-embed` (for init templates)

---

## Building and Running

The project uses a `Makefile` to orchestrate common development tasks.

### Core Commands
- **Build:** `make build` (runs `cargo build --release`)
- **Install:** `make install` (builds, symlinks to `~/.local/bin/forge`, and activates git hooks)
- **Test:** `make test` (runs validation and `cargo test`)
- **Validate:** `make validate` (runs pre-commit checks via `.githooks/pre-commit`)
- **Clean:** `make clean` (removes build artifacts)

### CLI Usage Examples
- **Full Install:** `forge install <module_path>`
- **Assemble Only:** `forge assemble <module_path>`
- **Deploy Only:** `forge deploy <module_path>`
- **Validate Module:** `forge validate <module_path>`
- **Check for Drift:** `forge drift <local_path> <upstream_path>`

---

## Architecture & Code Structure

### Directory Map
- `src/`: Core logic
    - `main.rs`: Binary entry point, dispatches to `cli/`.
    - `commands.rs`: Library entry point (`commands` crate).
    - `error.rs`: Custom `Error` and `ErrorKind` implementation.
    - `cli/`: Subcommand implementations (one folder per command).
    - `assemble/`: Logic for merging variants and stripping frontmatter/links.
    - `transform/`: Provider-specific transformations (kebab-case, remap-tools, to-toml).
    - `manifest/`: Manifest read/write and SLSA provenance generation.
    - `validate/`: Schema and structural validation.
    - `yaml/`: Deep merge logic for configuration files.
- `templates/init/`: Embedded templates used by `forge init`.
- `schemas/`: `.mdschema` files for document validation.
- `docs/decisions/`: ADRs (Architecture Decision Records) detailing technical choices.

### Error Handling
- Uses a custom `Error` struct with an `ErrorKind` enum (`Parse`, `Config`, `Io`, `Deploy`, `Validate`).
- Prefers `Result<T, Error>` for structured error propagation.

---

## Development Conventions

### Coding Style
- **Indentation:** 4-space indentation (configured in `rustfmt.toml`).
- **Lints:** `#[forbid(unsafe_code)]` and strict `clippy` pedantic lints are enforced.
- **Naming:** Follows standard Rust conventions (`PascalCase` for types, `snake_case` for functions/variables).

### Testing Practices
- **Unit Tests:** Sibling `tests.rs` files or `mod tests` blocks within modules.
- **Integration Tests:** Located in the `tests/` directory at the project root.
- **Fixtures:** Fixtures for testing are stored in `tests/fixtures/`.

### Contribution Workflow
- All changes require PRs and must pass `make validate`.
- Pre-commit hooks are mandatory and configured to use `forge` itself for validation.
- **`defaults.yaml`** is the source of truth for configuration.

## Transformation Guide

When deploying to Gemini (`.gemini/`), `forge-cli` automatically transforms content to match Gemini CLI conventions:

### 1. Tool Name Remapping
`PascalCase` tool names in backticks are remapped to their `snake_case` equivalents using `config/remap-tools.yaml`.
- `` `Read` `` becomes `` `read_file` ``
- `` `Bash` `` becomes `` `run_shell_command` ``
- `` `Complete` `` becomes `` `complete_task` ``

### 2. Nested Pathing
- **Agents:** Filenames are converted to kebab-case (e.g., `agents/SecurityArchitect.md` → `agents/security-architect.md`).
- **Skills & Rules:** Directory structures are preserved to maintain module identity (e.g., `skills/MySkill/SKILL.md` → `skills/MySkill/SKILL.md`).
