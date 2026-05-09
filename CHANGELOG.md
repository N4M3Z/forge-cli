# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/).

## [Unreleased]

### Fixed

- `forge init` now deploys all hidden template files (`.pre-commit-config.yaml`, `.gitattributes`, `.gitleaks.toml`, `.gitlab-ci.yml`). The previous near-total dotfile allowlist silently dropped them; replaced with an OS-junk blocklist (`.DS_Store`, `Thumbs.db`, `Desktop.ini`, `._*` resource forks). (#28)
- `templates/init/.pre-commit-config.yaml` ruff hook drops `pass_filenames: false`, which was bypassing the `types: [python]` filter and forcing ruff to run on every commit (including markdown-only modules without ruff installed). With the flag gone, prek skips the hook when no Python files are staged. (#33)
- forge-cli's own root `.pre-commit-config.yaml` drops `--no-git -s .` from the gitleaks entry. The flag bypassed git's gitignore, walking 4 GB of cargo `target/` and hanging at 400% CPU. Default invocation respects gitignore.

### Added

- `forge copy` writes SLSA provenance sidecars to `.provenance/` in the target tree (opt-out via `--skip-provenance`)
- `forge drift` consumes copy provenance sidecars to surface source URI on same-name matches and pair files across renames
- `forge install` and `forge deploy` accept `--provider <NAME>` (repeatable) to deploy only the named provider(s); unknown names error with the available list
- `forge install`, `forge deploy`, and `forge clean` default the source path to `.` when `--source` is omitted

### Changed

- `manifest::generate_statement` builds the SLSA statement via typed `serde_yaml::to_string` (eliminates YAML injection risk in interpolated fields)
- Copy provenance subject names and dependency URIs use POSIX path separators regardless of host OS
- `forge install`, `forge deploy`, `forge clean` refuse to operate on a directory without `module.yaml`; the error names the missing file and the corrective `--source` invocation
- The YAML deep-merge "type conflict" warning now identifies the conflicting key path and the involved YAML types
- `forge install --help` lists the available providers, explains the `--target` per-provider join, and shows two example invocations

### Removed

- All commands drop their positional path arguments. Same positional meant different things across verbs (`forge init <PATH>` wrote into PATH, `forge install <PATH>` read from PATH); every command now uses named flags (`--source`, `--target`, `--upstream`).
    - `install`, `deploy`, `clean`, `assemble`, `validate`, `release`: source is `--source <DIR>`, defaults to `.`
    - `init`: target is `--target <DIR>`, no default (scaffolding requires explicit destination)
    - `copy`: both `--source <DIR>` and `--target <DIR>` are required
    - `provenance`: inspection target is `--target <DIR_OR_FILE>` (defaults to `.`); the existing source-URI filter is renamed from `--source` to `--source-uri` to avoid name collision
    - `drift`: source defaults to `.` via `--source`; the second positional is now `--upstream <DIR>` (renamed from `target` since semantically it is the upstream reference)

## [0.3.1] - 2026-04-16

### Added

- Gemini CLI compatibility: tool remapping, `kebab-case-agents` rule, skill path preservation
- `GEMINI.md` provider overview for Gemini-side consumers
- Composite GitHub Action for CI integration (`.github/actions/setup-forge/`)
- `.gitleaks.toml` for excluding eval baselines from secret scanning
- GitLab CI template in `templates/init/`

### Changed

- `map_field` uses `serde_yaml` round-trip (handles quoted values and block scalars)
- Assembly transforms documented in README
- Heavy scanners (gitleaks, semgrep) moved to `pre-push` stage in init template

### Fixed

- Trailing newlines preserved during assembly (`.lines()` drop fix)
- Removed dead `_tool_mappings` parameter from assembly pipeline
- Removed forge-core-specific `validate-adr` hook from init template

## [0.3.0] - 2026-04-06

### Added

- `forge init` scaffolds new modules from embedded templates with SLSA provenance
- `forge validate` manifest-based drift detection against current templates
- `.pre-commit-hooks.yaml` makes forge-cli a valid prek hook source (`language: rust`)
- prek as declarative validation entry point
- Native YAML, JSON, and trailing whitespace checks in `forge validate`
- `--source` filter on `forge provenance` command

### Changed

- `templates/` reorganized: content schemas in `templates/init/`, build helpers in `templates/make/`

## [0.2.0] - 2026-04-04

### Added

- `forge drift` command for upstream comparison with frontmatter key diffing and `--ignore` flag
- `forge provenance --show-orphans` flag for detecting files without provenance
- `forge clean` command for removing stale files from previous installs
- `forge release` command for packaging assembled content as tarballs
- `forge validate` runs external tools (shellcheck, cargo fmt/clippy, cargo test, tsc, gitleaks)
- Skill `user/` subdirectory flattening during assembly (override semantics)
- mdschema templates for skills, agents, rules, and decisions (embedded via rust-embed)
- Hash-verified `validate.sh` fallback for pre-commit hooks and CI
- GitHub Actions release workflow for cross-platform binaries (Linux x86_64, macOS aarch64)
- `validate.yaml` and `git/pre-commit` templates for consumer modules
- 31 ADRs migrated to structured-madr frontmatter format
- JSON Schema files for frontmatter validation

### Changed

- `target::resolve_paths` returns `Result` instead of panicking
- Validation file lists hardcoded in binary, removed from `defaults.yaml`
- `ModuleManifest` typed struct for `module.yaml` deserialization
- `validate.sh` uses `git ls-files` to avoid submodule recursion
- Rust file walker skips git submodule directories (`.git` file detection)
- Gitleaks uses `protect --staged` when staged changes exist, `detect` otherwise

### Fixed

- Code fence content no longer misidentified as headings in mdschema validation
- ADR mdschema test uses inert fixture instead of live ADR file
- Graceful fallback when module config is incompatible with provider defaults

## [0.1.0] - 2026-03-25

### Added

- Two-stage assembly and deployment pipeline (assemble → deploy)
- Provider-specific transforms: kebab-case, tool remapping, TOML conversion
- SLSA/in-toto provenance sidecars (.yaml) in build/
- Deployment manifest (.manifest) at target for staleness detection
- Variant resolution with precedence: user/ > provider/model/ > provider/ > base
- Frontmatter stripping with configurable keep fields
- GFM reference link stripping
- Incremental install with user modification detection
- INSTALL.md following Mintlify install.md standard
- 28 ADRs documenting architecture decisions

[Unreleased]: https://github.com/N4M3Z/forge-cli/compare/v0.3.1...HEAD
[0.3.1]: https://github.com/N4M3Z/forge-cli/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/N4M3Z/forge-cli/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/N4M3Z/forge-cli/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/N4M3Z/forge-cli/releases/tag/v0.1.0
