# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/).

## [Unreleased]

### Added

- The `agentskills` provider installs Agent Skills-compatible `SKILL.md` files under `.agents/skills/<Name>/SKILL.md`, with `agents` as an alias for `--provider agents` and an Agent Skills frontmatter whitelist.
- `forge adopt <url>` fetches an upstream HTTPS artifact (or `file://` fixture), applies the `align` transform into a module skill or companion file, and writes an `adopt/v1` provenance sidecar with the upstream digest pin.
- `forge find "<query>"` scans local modules, discovered repos, and already-cached watchlist sources for skills, agents, and rules, ranking matches by name, trigger text, and description with optional JSON output.
- `forge install` now warns prominently and refuses by default when the source git checkout is confirmed behind local `origin/main` or `origin/master`; `--allow-stale` overrides the refusal while still printing the warning. Prune is qualifier-aware: a deployed base file whose source counterpart exists only under a qualifier directory is pruned, while a correctly-resolved qualifier deployment is kept. (#73)
- `forge config` prints the resolved forge ontology from `~/.config/forge/config.yaml`, with `FORGE_*` environment overrides taking precedence over file values and built-in defaults. The legacy `project.yaml` file remains as a deprecated fallback for one release.
- `forge exec <skill>` runs skill-bundled scripts through a small synchronous runtime table (`uv run`, `bash`, `deno run`, `node`), injects `FORGE_*`/`INPUT_*` environment, supports JSON stdin/stdout wrapping, dry-runs, and output schema validation.
- Unknown `forge <verb>` commands now dispatch to external `forge-<verb>` scripts from the module `commands/` directory, configured extension directories, or `PATH`, keeping new capabilities out of the Rust kernel.
- `forge tui` and bare `forge` under `--features tui` launch a ratatui terminal dashboard over the shared `commands::services` scan model, with artifacts, provenance, projects/ontology placeholder, sources/watch/find panes, and a command palette.
- `forge tui` now opens an instant Miller-column dashboard at full web-dashboard parity: sections, artifact lists, tabbed artifact detail, provenance chain/deploy groups, ADRs, variants, integrity attention list, search filters/sorts, git history, Settings/Hooks/Config/Schemas file-browser sections, and a `?` help overlay driven by the same keybinding table as the footer hints. Scans run on a background thread so the shell renders immediately while module discovery continues.
- Model-level qualifier resolution for rules and agents (PROV-0005 Phase 1). Assembly now resolves the full `user/` > `provider/<model>/` > `provider/` > base precedence: a file at `rules/<provider>/<model-id>/Rule.md` overrides the base for that provider and model, and the long-documented `user/` overlay is finally wired for rules and agents too. Each provider gains a default `model` in `defaults.yaml` (an exact ID from `config/models.yaml`); `forge assemble --model <ID>` and `forge install --model <ID>` override it for providers that list that model. Qualifier directory names are validated as exact model IDs: model IDs are no longer split into segments, so a directory named `4` or `6` (from `claude-opus-4-6`) is no longer a valid qualifier, and a model-only file under `rules/<provider>/<model-id>/` is collected instead of silently dropped. An unrecognized model-qualifier subdirectory is skipped with a warning rather than dropped silently. Skill overlays, recording the model in `.manifest`/provenance sidecars, and the per-model release matrix are deferred to Phase 2. (#60)
- `forge drift --target <BASE>` verifies a module's assembled `build/` against where it was deployed, scoped to the module's own files. It mirrors `forge install --target`: each provider's `build/<provider>` is compared to `<BASE>/<provider-target>`, so files built but not yet deployed surface as local-only, deployment edits surface as frontmatter/body drift, and this module's deployed files (per the target `.manifest` plus provenance attribution) that are no longer built surface as drift. Unlike `--upstream` against a multi-module tree such as `~/.claude`, other modules' files are never reported. `--target` and `--upstream` are mutually exclusive. (#61)
- `forge validate` sanity-checks Claude Code plugin scaffolding when `.claude-plugin/plugin.json` is present: each manifest (`plugin.json`, `.claude-plugin/marketplace.json`, `hooks/hooks.json`) must be valid JSON, and every hook script referenced via `${CLAUDE_PLUGIN_ROOT}` must exist and be executable (the most common cause of hooks silently not firing). It deliberately makes no assertions about the plugin or marketplace field schema, so it does not break when Claude Code's plugin format changes. Non-plugin modules are unaffected: the check runs only when `plugin.json` exists. (#59)
- `.forge` consumer manifests accept `git:` sources pinned to a 40-hex commit SHA. `forge install` clones the remote via `gix` into a content-addressed cache at `~/.cache/forge/git/<host>/<owner>/<repo>/`, materializes the pinned tree, and feeds it through the standard assemble + deploy pipeline. HTTPS-only; `ssh://`, `git://`, `git@host:` shorthand, and userinfo URLs are rejected at parse time. Branch names, tags, and abbreviated SHAs are rejected in favor of explicit 40-char commit hashes. Cache hits are instant; the bare clone is reused across SHA pins within the same repository. (#53)

### Changed

- Pure dashboard view builders for nested overview, matrix, variant coverage, deployment grouping, dependency links, and search sorting/filtering moved into `commands::services::builders` so the axum dashboard and TUI share the same rendering inputs.
- Dashboard scan logic moved from the dashboard binary into `commands::services`, shared by the axum dashboard and reusable by a future TUI with no behavioral change.
- `forge install` and `forge deploy` default `--target` to `--source` when a `.forge` consumer manifest is present and `--target` is omitted. The consumer dir IS the place the user wants provider trees written; the previous behavior forced redundant `--target .` on every consumer-mode invocation. Module-root flows (no `.forge`) are unchanged: an omitted `--target` still resolves provider directories relative to the current working directory. (#52)

### Deferred

- Line-level annotation export from the TUI Code tab remains follow-up work.

### Fixed

- Prune is qualifier-aware, so base files are removed when the only remaining source match is an inactive qualifier variant while correctly resolved qualifier output is kept. (closes #73)
- `forge provenance` verifies source-side `.provenance/` sidecars, not just deployed targets. Pointed at a source repository or an artifact subdirectory (detected by a `module.yaml` at or above the path), it walks `.provenance/*.yaml`, resolves each `subject.name` to a repo-relative file, recomputes its SHA-256 against the recorded digest, and verifies the digests of in-repo `resolvedDependencies` (the remote `upstream` dependency is left to its recorded pin). Previously the strict typed sidecar model rejected `adopt/v1` sidecars (which carry `upstream_url` rather than `source`) and the directory walk only iterated provider kind roots, so `forge provenance skills/Foo` reported "No provenance found" and source-side drift went undetected. The model now tolerates both `assemble/v1` and `adopt/v1` schemas without changing generated sidecar output, and `--json` emits a machine-readable per-sidecar report. (#44)
- Assembly preserves Claude-native `SKILL.md` frontmatter for the claude provider. `claude.keep_fields.skills` previously kept only `name`, `description`, and `version`, so `forge assemble` stripped `allowed-tools` (and the rest of the Claude Code optional skill fields) from deployed skills, breaking tool pre-approval and gating dynamic context injection (`!` command lines). The claude skill keep-list now mirrors the skill mdschema whitelist (`allowed-tools`, `argument-hint`, `arguments`, `disable-model-invocation`, `user-invocable`, `model`, `effort`, `context`, `agent`, `paths`, `shell`). The frontmatter stripper also retains multi-line block values for kept fields, so a list-form `allowed-tools:` survives intact instead of collapsing to a valueless key. (#69)

## [0.3.2] - 2026-05-22

### Fixed

- `forge install` now prunes deployed skill, agent, and rule directories absent from source. Stale directories (renamed, folded, or deleted upstream) used to keep loading into Claude / Gemini / Codex / OpenCode sessions, shadowing renames. Pruned content moves to `<target>/.trash/<UTC-ts>/` for recoverability; restore with `mv`, reclaim with `rm -rf`. Empty parent directories are walked and removed up to but not including the provider target root. Opt out with `--no-prune`; preview with `--dry-run`. Locally modified files (deployed SHA-256 no longer matches the manifest fingerprint) are skipped with a warning; pass `--force` to prune them anyway. `forge clean` uses the same quarantine path for consistency. The substring-collision bug in `is_owned_by_module` (which previously matched `Prompts` against `PublishPrompts` and let two modules named `forge-core` at different repositories prune each other's files) is fixed via structured `(host, owner, repo)` equality on the source URI. (#45)
- `templates/init/.githooks/pre-commit` no longer ships a hand-typed `SCRIPT_SHA` constant. `build.rs` computes `sha256(scripts/validate.sh)` at compile time and emits it as `commands::VALIDATE_SH_SHA`; `forge init` substitutes `${VALIDATE_SH_SHA}` into the template at scaffold time. The pin can no longer go stale: any change to `scripts/validate.sh` triggers a rebuild that ripples through to every fresh `forge init`. `tests/embedded_sha.rs` pins the contract; `src/cli/tests.rs` dispatches every `forge install` / `forge validate` template invocation through clap to catch CLI-shape drift before it ships; a new `template-smoke` CI job scaffolds and runs `make validate` against the generated tree on every PR. (#47, #46)
- `templates/init/Makefile` drops the stale trailing `.` from `$(FORGE) install .`. The current CLI uses `--source` (default `.`) and rejects positional args, so the previous template made `make install` fail on every freshly scaffolded module. (#46)
- `markdown_to_toml` now serializes Codex agent `.toml` via the `toml` crate, which picks a safe string form (`"..."`, `"""..."""`, or `'''...'''`) based on body content. The previous implementation interpolated the body directly into a `"""..."""` literal with no escaping, so a body containing `"""\n[section]\nkey = "x"` could break out of the literal and inject arbitrary top-level tables into the deployed agent config. (#43)
- `forge provenance --target <DIR>` now walks every deployed content file regardless of extension instead of only `.md`. The codex provider produces `.toml` agent files, so the previous `.md`-only filter caused `forge provenance --target ~/.codex` to report "No provenance found" even when every sidecar matched. Sidecars (`.yaml`) and dotfiles (`.DS_Store`, `.manifest`) are still skipped. (#29)
- `forge init` now deploys all hidden template files (`.pre-commit-config.yaml`, `.gitattributes`, `.gitleaks.toml`, `.gitlab-ci.yml`). The previous near-total dotfile allowlist silently dropped them; replaced with an OS-junk blocklist (`.DS_Store`, `Thumbs.db`, `Desktop.ini`, `._*` resource forks). (#28)
- `templates/init/.pre-commit-config.yaml` ruff hook drops `pass_filenames: false`, which was bypassing the `types: [python]` filter and forcing ruff to run on every commit (including markdown-only modules without ruff installed). With the flag gone, prek skips the hook when no Python files are staged. (#33)
- forge-cli's own root `.pre-commit-config.yaml` drops `--no-git -s .` from the gitleaks entry. The flag bypassed git's gitignore, walking 4 GB of cargo `target/` and hanging at 400% CPU. Default invocation respects gitignore.
- `templates/init/skills/.mdschema` whitelists 12 Claude Code optional `SKILL.md` frontmatter fields (`when_to_use`, `argument-hint`, `arguments`, `allowed-tools`, `disable-model-invocation`, `user-invocable`, `model`, `effort`, `context`, `agent`, `paths`, `shell`). Modules that lift those fields from a `SKILL.yaml` sidecar to top-level frontmatter (the natural authoring path now that Claude Code parses them natively) no longer fail validation with `Unknown frontmatter field`. `hooks` is omitted because mdschema lacks an `object` type for the nested map. (#40)

### Added

- `forge install` reads a `.forge` consumer manifest from `--source` when present, deploying only the artifacts the manifest lists. A consumer repo (not itself a forge module) declares which skills, agents, and rules it wants from which producer modules; `forge install` walks each declared local-path source on disk, filters its content to the requested subset, and runs the standard assemble + deploy pipeline scoped to the consumer's own `.claude/`, `.gemini/`, `.codex/`, `.opencode/` directories. The schema is grouped by source: each entry under `sources:` names a module path, each entry under `artifacts:` lists requested skills/agents/rules per source. Git-URL sources, lockfiles, and plugin auto-enable are deferred to follow-up issues; this iteration supports local-path sources only. (#39)
- `forge copy` writes SLSA provenance sidecars to `.provenance/` in the target tree (opt-out via `--skip-provenance`)
- `forge drift` consumes copy provenance sidecars to surface source URI on same-name matches and pair files across renames
- `forge install` and `forge deploy` accept `--provider <NAME>` (repeatable) to deploy only the named provider(s); unknown names error with the available list
- `forge install`, `forge deploy`, and `forge clean` default the source path to `.` when `--source` is omitted

### Changed

- Codex agent `.toml` files now include `name`, `model`, and `model_reasoning_effort` fields alongside `description`, and the body is emitted as `developer_instructions` (renamed from `instructions`). The codex provider also defines `effort` tiers (`strong → medium`, `fast → low`, `light → low`) and extends `keep_fields.agents` to retain `model` and `effort` so they flow through assembly. Consumers must rerun `forge install` for deployed agents to pick up the new field names. Per the OpenAI Responses API the `developer` role outranks `user`, so adopted upstream agent content now inherits that elevated trust on Codex. (#43)
- `manifest::generate_statement` builds the SLSA statement via typed `serde_yaml::to_string` (eliminates YAML injection risk in interpolated fields)
- Copy provenance subject names and dependency URIs use POSIX path separators regardless of host OS
- `forge install`, `forge deploy`, `forge clean` refuse to operate on a directory without `module.yaml`; the error names the missing file and the corrective `--source` invocation
- The YAML deep-merge "type conflict" warning now identifies the conflicting key path and the involved YAML types
- `forge install --help` lists the available providers, explains the `--target` per-provider join, and shows two example invocations
- Codex default models refreshed to currently-supported GPT-5 Codex variants in `defaults.yaml` and `config/models.yaml`. (#41, #51)

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

[Unreleased]: https://github.com/N4M3Z/forge-cli/compare/v0.3.2...HEAD
[0.3.2]: https://github.com/N4M3Z/forge-cli/compare/v0.3.1...v0.3.2
[0.3.1]: https://github.com/N4M3Z/forge-cli/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/N4M3Z/forge-cli/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/N4M3Z/forge-cli/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/N4M3Z/forge-cli/releases/tag/v0.1.0
