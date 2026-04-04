# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/).

## [Unreleased]

### Added

- `forge drift` command for upstream comparison with frontmatter key diffing and `--ignore` flag
- `forge provenance --orphans` flag for detecting files without provenance
- `forge provenance --source` filter for scoping scan results by module
- `forge release` command for packaging assembled content as tarballs
- `forge validate` runs external tools (shellcheck, cargo, tsc, gitleaks) when available
- `ModuleManifest` typed struct for `module.yaml` deserialization
- Stale file detection and `--prune` flag for install/deploy (#1)
- Qualifier directory support for rules and agents (CORE-0018)
- Model tier matching via models.yaml (e.g., `sonnet/` matches claude and opencode)
- Embedded models.yaml fallback for standalone binary usage

### Changed

- `target::resolve_paths` returns `Result` instead of panicking
- Validation file lists hardcoded in binary, removed from `defaults.yaml`

### Fixed

- ADR mdschema test uses inert fixture instead of live ADR file

## [0.1.0] - 2026-03-25

### Added

- Two-stage assembly and deployment pipeline (assemble → copy)
- Provider-specific transforms: kebab-case, tool remapping, TOML conversion
- SLSA/in-toto provenance sidecars (.yaml) in build/
- Deployment manifest (.manifest) at target for staleness detection
- Variant resolution with precedence: user/ > provider/model/ > provider/ > base
- Frontmatter stripping with configurable keep fields
- GFM reference link stripping
- Incremental install with user modification detection
- INSTALL.md following Mintlify install.md standard
- 28 ADRs documenting architecture decisions
