# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/).

## [Unreleased]

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
