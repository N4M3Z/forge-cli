---
status: Accepted
date: 2026-03-19
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["DeveloperCouncil", "WebResearcher"]
informed: []
tags: [assembly, deployment, architecture]
---

# Assembly and Deployment Pipeline

## Context and Problem Statement

Skills, agents, and rules are authored as markdown with YAML frontmatter. Each AI coding provider expects files in different directories, with different naming conventions, different body formats, and different metadata. The system needs a clear separation between content transformation (assembly) and file placement (deployment).

## Decision Drivers

- Authors write once, deploy to multiple providers
- Assembly transforms are deterministic and testable in isolation
- Deployment is a commodity operation (file copy) that external tools like rulesync already handle
- The pipeline must produce a build artifact that can be inspected before deployment

## Decision Outcome

A two-stage pipeline with an intermediate `build/` directory:

```
source/         -->    assemble    -->    build/          -->    provider dirs
(authored)             (transform)                  (assembled)           (deployed)
```

### Stage 1: Assembly (forge-cli)

Transforms source content into provider-specific output:

1. Parse frontmatter — extract name, targets, description, model, tools
2. Resolve variant — check qualifier directories (user/ > provider/model/ > provider/ > base)
3. Merge — combine base + variant body using variant's `mode` (append, prepend, replace)
4. Strip frontmatter — remove `---` delimiters, H1 heading from body
5. Strip reference links — remove `[N]: url` definitions and `[N]` inline markers
6. Format per provider — YAML frontmatter (Claude/Gemini/OpenCode), TOML (Codex), kebab-case names (Gemini/OpenCode), tool remapping (Gemini)
7. Write sidecar — `.yaml` companion preserving stripped frontmatter + provenance

Output structure:

Source (repository — qualifier directories for variant resolution):

```
repository/
    rules/
        MyRule.md                                   base (provider-agnostic)
        claude/MyRule.md                            claude-specific variant
        claude/claude-opus-4-6/MyRule.md            model-specific variant
        codex/MyRule.md                             codex-specific variant
        codex/o4-mini/MyRule.md                     model-specific variant
        user/MyRule.md                              user override (highest priority)
    agents/
        MyAgent.md                                  base
        claude/MyAgent.md                           claude variant
        gemini/my-agent.md                          gemini variant
        codex/MyAgent.toml                          codex variant
        user/MyAgent.md                             user override
    skills/
        MySkill/SKILL.md                            base
        MySkill/Reference.md                        companion (passthrough)
        MySkill/claude/SKILL.md                     claude-specific variant
        MySkill/gemini/SKILL.md                     gemini-specific variant
        MySkill/user/SKILL.md                       user override of SKILL.md
        MySkill/user/ForgeADR.md                    user-only companion (flattened)
```

Resolution precedence (highest first): `user/` > `provider/model/` > `provider/` > base.

This applies uniformly to all content kinds including skill companions. Subdirectories are flattened at assembly — the prefix is stripped from the output path:

```
SOURCE                               ASSEMBLED (build/claude/)           DEPLOYED (.claude/)
────────────────────────────         ────────────────────────────        ────────────────────────────
skills/ArchitectureDecision/         skills/ArchitectureDecision/        skills/ArchitectureDecision/
├── SKILL.md                    ──→  ├── SKILL.md (pipeline)        ──→  ├── SKILL.md
├── TemplateReference.md        ──→  ├── TemplateReference.md       ──→  ├── TemplateReference.md
├── SchemaValidation.md         ──→  ├── SchemaValidation.md        ──→  ├── SchemaValidation.md
└── user/                            ├── ForgeADR.md  ← flattened   ──→  ├── ForgeADR.md
    ├── ForgeADR.md             ──→  └── ContextKeeper.md           ──→  └── ContextKeeper.md
    └── ContextKeeper.md        ──→
```

When a file exists both at the root and in `user/`, the `user/` version wins (override):

```
SOURCE                               ASSEMBLED (build/claude/)
────────────────────────────         ────────────────────────────
skills/MySkill/                      skills/MySkill/
├── SKILL.md                    ──→  ├── SKILL.md (pipeline)
├── Reference.md                ─╳   ├── Reference.md ← user/ wins
└── user/                            └──
    └── Reference.md            ──→
```

Assembled output (variants resolved, frontmatter stripped, ready to deploy):

```
build/
    claude/
        rules/MyRule.md                             assembled
        rules/MyRule.yaml                           sidecar (stripped frontmatter + provenance)
        agents/MyAgent.md                           YAML frontmatter format
        agents/MyAgent.yaml                         sidecar (stripped frontmatter + provenance)
        skills/MySkill/SKILL.md                     frontmatter stripped
        skills/MySkill/SKILL.yaml                   sidecar (stripped frontmatter + provenance)
    gemini/
        rules/my-rule.md                            assembled, kebab-case
        rules/my-rule.yaml                          sidecar (stripped frontmatter + provenance)
        agents/my-agent.md                          kebab-case, tools remapped
        agents/my-agent.yaml                        sidecar (stripped frontmatter + provenance)
    codex/
        agents/MyAgent.toml                         TOML format
        agents/MyAgent.yaml                         sidecar (stripped frontmatter + provenance)
```

Deployment copies content files from `build/claude/` → `.claude/`. Sidecars (`.yaml`) stay in `build/`. Deployment writes a `.manifest` dotfile in each target directory recording the SHA-256 of each deployed file (see ASSEMBLY-0003).

### Stage 2: Deployment (file copy)

Copies assembled files from `build/<provider>/` to `.<provider>/`. This is a flat copy operation with no transformation.

If rulesync is available, it handles deployment to 21+ providers. If not, a minimal deployer reads the provider config (prefix, extension) and copies files. Either way, assembly output is the same.

### Example: Rule with variant

```
rules/UseRTK.md                rules/user/UseRTK.md           build/rules/UseRTK.md
(base)                         (user variant)                  (assembled)
┌────────────────────┐         ┌────────────────────┐          ┌────────────────────┐
│ ---                │         │ ---                │          │ Always prefix      │
│ name: UseRTK       │ resolve │ mode: append       │  merge   │ shell commands     │
│ ---                │ ─────>  │ ---                │ ─────>   │ with `rtk`.        │
│ Always prefix      │ variant │ RTK does not       │          │                    │
│ shell commands     │         │ support -C.        │          │ RTK does not       │
│ with `rtk` [1].   │         └────────────────────┘          │ support -C.        │
│                    │                                         └────────────────────┘
│ [1]: https://...   │         strip fm + refs
└────────────────────┘         ─────────────>
```

### Example: Agent per provider

```
agents/SecurityArchitect.md       build/claude/agents/SecurityArchitect.md    (YAML frontmatter)
(source)                          build/gemini/agents/security-architect.md   (kebab + tool remap)
                                  build/codex/agents/SecurityArchitect.toml   (TOML format)
```

### Consequences

- [+] Assembly is a pure function — testable without filesystem side effects
- [+] `build/` directory is inspectable before deployment
- [+] Deployment is decoupled — replaceable by rulesync, native CLIs, or `forge copy`
- [+] Provenance tracks the full transform chain
- [-] Two-stage adds a build step vs. direct copy (direct copy remains as fallback per ASSEMBLY-0009)
