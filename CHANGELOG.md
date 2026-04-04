# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/).

## [Unreleased]

### Added

- Stale file detection and `--prune` flag for install/deploy (#1)
- Qualifier directory support for rules and agents (CORE-0018)
- Model tier matching via models.yaml (e.g., `sonnet/` matches claude and opencode)
- Embedded models.yaml fallback for standalone binary usage

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
