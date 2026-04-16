# forge-cli

Assemble, validate, and deploy markdown content across AI coding providers.

Skills, agents, and rules are authored once as markdown with YAML frontmatter. forge-cli transforms them for each provider's conventions and deploys to the right directories.

## Why not just copy files?

Copying works until instructions drift. forge-cli adds three things raw copying can't:

- **Assembly** — strips frontmatter, resolves `user/` overrides, applies provider-specific transforms (kebab-case, tool remapping). The deployed file is clean; the source keeps its metadata.
- **Provenance** — each deployed file has an [in-toto/SLSA][6] record of what sources produced it. When something breaks, you can trace which source file and which override combined to produce the deployed instruction.
- **Manifest tracking** — `.manifest` at each target records what was deployed and when. Detects user modifications, skips unchanged files, prunes orphans from renamed sources.

The `user/` subdirectory lets individuals customize without polluting upstream (git-ignored, merged during assembly). Model qualifier directories (`claude-opus-4/`, `claude-sonnet-4/`) handle the reality that models need different instructions as they evolve.

## What it does

**Assemble** — Transforms source markdown into provider-specific output. Strips frontmatter, removes GFM reference links, resolves variant overrides, applies provider rules (kebab-case filenames, tool name remapping, TOML conversion). Writes provenance sidecars (SLSA/in-toto) alongside each built file.

**Deploy** — Deploys assembled files from `build/` to provider target directories. Tracks deployments via `.manifest` dotfiles for incremental installs — skips unchanged files, detects user modifications, overwrites stale content.

**Install** — Runs assemble + deploy in one step.

**Validate** — Checks module structure, `.mdschema` compliance, and runs external tools (shellcheck, cargo fmt/clippy, cargo test, tsc, gitleaks) when available.

**Drift** — Compares a module's content against an upstream reference. Separates frontmatter from body, reports which keys changed, supports `--ignore` for expected per-project differences.

**Provenance** — Shows the source-to-deployed chain for a file, or scans a directory for verification status grouped by source module.

**Copy** — Copies source files directly to a target directory without assembly or transforms. No manifest tracking.

**Clean** — Removes stale files from previous installs. Compares the current build against deployed targets and deletes files no longer in the module.

**Release** — Packages assembled content as release tarballs.

## How Content Flows

```ascii
  SOURCE                         ASSEMBLE                          DEPLOY
  ┌──────────────────────┐       ┌────────────────────────────┐      ┌──────────────────────┐
  │ rules/               │       │ build/                     │      │ .claude/             │
  │ ├── UseRTK.md        │──┐    │ ├── claude/                │──┐   │ ├── rules/           │
  │ ├── claude/UseRTK.md │──┤    │ │   ├── rules/             │  │   │ │   └── UseRTK.md    │
  │ └── user/UseRTK.md   │──┘    │ │   │   └── UseRTK.md      │  ├──→│ ├── agents/          │
  │                      │  ┌──→ │ │   ├── agents/            │  │   │ │   └── GameMaster.md│
  │ agents/              │  │    │ │   │   └── GameMaster.md  │  │   │ ├── skills/          │
  │ └── GameMaster.md    │──┘    │ │   └── skills/            │  │   │ │   └── MySkill/     │
  │                      │       │ │       └── MySkill/       │  │   │ │       ├── SKILL.md │
  │ skills/              │       │ │           ├── SKILL.md   │  │   │ │       ├── Ref.md   │
  │ └── MySkill/         │──┐    │ │           ├── Ref.md     │  │   │ │       └── Extra.md │
  │     ├── SKILL.md     │──┤    │ │           └── Extra.md   │  │   │ └── .manifest        │
  │     ├── Ref.md       │──┤    │ │               ↑          │  │   └──────────────────────┘
  │     └── user/        │  │    │ │           flattened from │  │
  │         └── Extra.md │──┘    │ │           user/          │  │   ┌─────────────────────┐
  └──────────────────────┘       │ ├── gemini/                │  ├──→│ .gemini/            │
                                 │ │   └── ... (kebab-case)   │  │   └─────────────────────┘
       ┌──────────────┐          │ ├── codex/                 │  │
       │ Strip:       │          │ │   └── ... (TOML agents)  │  │   ┌─────────────────────┐
       │  frontmatter │          │ └── opencode/              │  └──→│ .codex/             │
       │  ref links   │          │     └── ... (kebab-case)   │      └─────────────────────┘
       │ Resolve:     │          └────────────────────────────┘
       │  variants    │
       │  qualifiers  │          ┌──────────────┐
       │ Generate:    │          │ .yaml prov   │  provenance sidecars
       │  sidecars    │          │ .manifest    │  deployment tracking
       └──────────────┘          └──────────────┘
```

### Qualifier Directories

Subdirectories in source are organizational — they flatten at assembly time:

| Directory         | Purpose                      | Precedence |
| ----------------- | ---------------------------- | ---------- |
| `user/`           | User overrides and additions | Highest    |
| `provider/model/` | Model-specific variants      |            |
| `provider/`       | Provider-specific variants   |            |
| *(root)*          | Base content                 | Lowest     |

When a file exists in both `user/` and root, `user/` wins. Files only in `user/` are deployed flat alongside root files.

## Providers

Provider conventions are config-driven via `defaults.yaml` (optional; falls back to embedded defaults if missing):

```yaml
providers:
    claude:
        target: ".claude"
    gemini:
        target: ".gemini"
        aliases:
            - geminicli
        assembly:
            - kebab-case-agents
            - remap-tools
            - strip-links
    codex:
        target: ".codex"
        assembly:
            - agents-to-toml
            - strip-links
        deploy:
            - rulesync
    opencode:
        target: ".opencode"
        assembly:
            - kebab-case-agents
            - strip-links
```

## Usage

Assemble and deploy a module to all provider directories:

```sh
forge install path/to/module
```

Deploy to a specific directory:

```sh
forge install path/to/module --target ~/project
```

Overwrite user-modified files:

```sh
forge install path/to/module --force
```

Remove stale files from previous installs:

```sh
forge clean path/to/module
```

Build only, no deployment:

```sh
forge assemble path/to/module
```

Deploy from an existing build/ directory:

```sh
forge deploy path/to/module
```

Validate module structure, schemas, linters, and tests:

```sh
forge validate path/to/module
```

Compare a module against an upstream reference:

```sh
forge drift . ../forge-core
```

Suppress expected per-project frontmatter keys:

```sh
forge drift . ../forge-core --ignore project,author
```

Show provenance chain for a deployed file:

```sh
forge provenance ~/.claude/rules/UseRTK.md
```

Scan a directory for files without provenance:

```sh
forge provenance ~/.claude --show-orphans
```

Copy source files directly without assembly:

```sh
forge copy path/to/module --target ~/project
```

Package assembled content as tarballs:

```sh
forge release path/to/module
```

All commands support `--json` for machine-readable output.

## Assembly Transforms

Assembly rules transform content for each provider. Configured in `defaults.yaml` under `assembly:`.

| Rule                 | Scope          | Effect                                                             |
| -------------------- | -------------- | ------------------------------------------------------------------ |
| `kebab-case`         | all kinds      | Filenames to kebab-case, `name:` frontmatter to kebab-case         |
| `kebab-case-agents`  | agents only    | Same as `kebab-case` but restricted to agent files                 |
| `remap-tools`        | all kinds      | Replace tool names in backtick spans (e.g. `Read` to `read_file`) |
| `strip-links`        | all kinds      | Remove GFM reference-style link definitions                        |
| `agents-to-toml`     | agents only    | Convert markdown agent to TOML format                              |

## Build

```sh
make build      # cargo build --release
make install    # build, symlink to ~/.local/bin/forge, activate git hooks
make validate   # run pre-commit checks (prek → forge → validate.sh)
make test       # validate + cargo test
make clean      # remove build artifacts
```

## Pipeline Artifacts

| Artifact         | Stage    | Location            | Purpose                              |
| ---------------- | -------- | ------------------- | ------------------------------------ |
| `.yaml` sidecars | assemble | `build/<provider>/` | SLSA/in-toto source-to-output record |
| `.provenance/`   | deploy   | `.<provider>/`      | Provenance alongside deployed files  |
| `.manifest`      | deploy   | `.<provider>/`      | Fingerprint of each deployed file    |

See `docs/decisions/` for architectural decision records.

## License

[EUPL-1.2](LICENSE)

[6]: https://in-toto.io/
