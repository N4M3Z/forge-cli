# forge-cli

Assemble, validate, and deploy markdown content across AI coding providers.

Skills, agents, and rules are authored once as markdown with YAML frontmatter. forge-cli transforms them for each provider's conventions and deploys to the right directories.

## What it does

**Assemble** — Transforms source markdown into provider-specific output. Strips frontmatter, removes GFM reference links, resolves variant overrides, applies provider rules (kebab-case filenames, tool name remapping, TOML conversion). Writes provenance sidecars (SLSA/in-toto) alongside each built file.

**Copy** — Deploys assembled files from `build/` to provider target directories. Tracks deployments via `.manifest` dotfiles for incremental installs — skips unchanged files, detects user modifications, overwrites stale content.

**Install** — Runs assemble + copy in one step.

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
```

## Build

```sh
make build    # cargo build --release
make test     # cargo test + doc tests
make lint     # cargo fmt --check + clippy + semgrep
make install  # symlink to ~/.local/bin/forge
```

## Architecture

Two-stage pipeline with an intermediate `build/` directory:

```
                    ┌────────────────────────────────────────────────────┐
                    │                    build/                          │
  ┌──────────┐      │  ┌────────────────┐   ┌────────────────────────┐   │     ┌──────────────┐
  │  source/ │────> │  │ claude/        │   │ gemini/                │   │────>│ .claude/     │
  │          │      │  │   agents/      │   │   agents/              │   │     │   agents/    │
  │ agents/  │      │  │   rules/       │   │   rules/               │   │     │   rules/     │
  │ rules/   │      │  │   skills/      │   │   skills/              │   │     │   skills/    │
  │ skills/  │      │  │                │   │                        │   │     │   .manifest  │
  │          │      │  │ + .yaml prov       │ + .yaml prov           │   │     └──────────────┘
  └──────────┘      │  └────────────────┘   └────────────────────────┘   │     ┌──────────────┐
                    │                                                    │────>│ .gemini/     │
     assemble       │  codex/                opencode/                   │     │   .manifest  │
                    │  + .yaml prov          + .yaml prov                │     └──────────────┘
                    └────────────────────────────────────────────────────┘          copy
```

| Artifact     | Stage    | Location              | Purpose                              |
| ------------ | -------- | --------------------- | ------------------------------------ |
| `.yaml` prov | assemble | `build/<provider>/`   | SLSA/in-toto source-to-output record |
| `.manifest`  | copy     | `.<provider>/`        | SHA-256 of each deployed file        |

See `docs/decisions/` for architectural decision records.

## License

EUPL-1.2
