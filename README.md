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

Provider conventions are config-driven via `defaults.yaml`:

```yaml
providers:
    claude:
        target: ".claude"
    gemini:
        target: ".gemini"
        assembly:
            - kebab-case
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
            - kebab-case
            - strip-links
```

## Usage

```sh
forge install path/to/module                     # assemble + deploy to all providers
forge install path/to/module --target ~/project  # deploy to specific directory
forge install path/to/module --force             # overwrite user-modified files
forge install path/to/module --prune             # remove stale files from previous installs
forge assemble path/to/module                    # build only, no deployment
forge deploy path/to/module                      # deploy from existing build/
forge validate path/to/module                    # structure + mdschema + linters + tests
forge drift . --upstream ../forge-core           # compare against upstream reference
forge drift . --upstream ../forge-core --ignore project,author  # suppress expected keys
forge provenance ~/.claude/rules/UseRTK.md       # show provenance chain for a file
forge provenance ~/.claude --orphans             # scan directory, show files without provenance
forge copy path/to/module --target ~/project     # raw copy, no assembly or transforms
forge release path/to/module                     # package as tarballs
```

All commands support `--json` for machine-readable output.

## Build

```sh
make build    # cargo build --release
make test     # cargo test + doc tests
make lint     # cargo fmt --check + clippy + semgrep
make install  # symlink to ~/.local/bin/forge
```

## Pipeline Artifacts

| Artifact         | Stage    | Location            | Purpose                              |
| ---------------- | -------- | ------------------- | ------------------------------------ |
| `.yaml` sidecars | assemble | `build/<provider>/` | SLSA/in-toto source-to-output record |
| `.provenance/`   | deploy   | `.<provider>/`      | Provenance alongside deployed files  |
| `.manifest`      | deploy   | `.<provider>/`      | Fingerprint of each deployed file    |

See `docs/decisions/` for architectural decision records.

## License

EUPL-1.2

[6]: https://in-toto.io/
