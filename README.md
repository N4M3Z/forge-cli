# forge-cli

Assemble, validate, and deploy markdown content across AI coding providers.

Skills, agents, and rules are authored once as markdown with YAML frontmatter. forge-cli transforms them for each provider's conventions and deploys to the right directories.

## What it does

**Assemble** — Transforms source markdown into provider-specific output. Strips frontmatter, removes GFM reference links, resolves variant overrides, applies provider rules (kebab-case filenames, tool name remapping, TOML conversion). Writes provenance sidecars (SLSA/in-toto) alongside each built file.

**Copy** — Deploys assembled files from `build/` to provider target directories. Tracks deployments via `.manifest` dotfiles for incremental installs — skips unchanged files, detects user modifications, overwrites stale content.

**Install** — Runs assemble + copy in one step.

## How Content Flows

```
  SOURCE                         ASSEMBLE                          DEPLOY
  ┌──────────────────────┐       ┌──────────────────────────┐      ┌─────────────────────┐
  │ rules/               │       │ build/                   │      │ .claude/            │
  │ ├── UseRTK.md        │──┐    │ ├── claude/              │──┐   │ ├── rules/          │
  │ ├── claude/UseRTK.md │──┤    │ │   ├── rules/           │  │   │ │   └── UseRTK.md   │
  │ └── user/UseRTK.md   │──┘    │ │   │   └── UseRTK.md    │  ├──→│ ├── agents/         │
  │                      │  ┌──→ │ │   ├── agents/          │  │   │ │   └── GameMaster.md│
  │ agents/              │  │    │ │   │   └── GameMaster.md │  │   │ ├── skills/         │
  │ └── GameMaster.md    │──┘    │ │   └── skills/          │  │   │ │   └── MySkill/     │
  │                      │       │ │       └── MySkill/      │  │   │ │       ├── SKILL.md │
  │ skills/              │       │ │           ├── SKILL.md   │  │   │ │       ├── Ref.md   │
  │ └── MySkill/         │──┐    │ │           ├── Ref.md     │  │   │ │       └── Extra.md │
  │     ├── SKILL.md     │──┤    │ │           └── Extra.md   │  │   │ └── .manifest       │
  │     ├── Ref.md       │──┤    │ │               ↑          │  │   └─────────────────────┘
  │     └── user/        │  │    │ │           flattened from  │  │
  │         └── Extra.md │──┘    │ │           user/           │  │   ┌─────────────────────┐
  └──────────────────────┘       │ ├── gemini/               │  ├──→│ .gemini/            │
                                 │ │   └── ... (kebab-case)   │  │   └─────────────────────┘
       ┌──────────────┐          │ ├── codex/                │  │
       │ Strip:       │          │ │   └── ... (TOML agents)  │  │   ┌─────────────────────┐
       │  frontmatter │          │ └── opencode/             │  └──→│ .codex/             │
       │  ref links   │          │     └── ... (kebab-case)   │      └─────────────────────┘
       │ Resolve:     │          └──────────────────────────┘
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
    codex:
        target: ".codex"
        assembly:
            - agents-to-toml
    opencode:
        target: ".opencode"
        assembly:
            - kebab-case
```

## Usage

```sh
forge install path/to/module                    # assemble + deploy to provider dirs
forge install path/to/module --target ~/project # deploy to specific directory
forge install path/to/module --force            # overwrite user-modified files
forge assemble path/to/module                   # build only, no deployment
forge copy path/to/module --target ~/project    # deploy from existing build/
forge validate path/to/module                   # check module structure
```

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
