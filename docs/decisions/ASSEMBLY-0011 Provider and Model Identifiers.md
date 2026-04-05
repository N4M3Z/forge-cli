---
title: "Provider and Model Identifiers"
description: "Configurable models.yaml for validating qualifier directory names against provider APIs"
type: adr
category: assembly
tags:
    - assembly
    - providers
    - validation
status: accepted
created: 2026-03-19
updated: 2026-03-19
author: "@N4M3Z"
project: forge-cli
related: []
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["WebResearcher"]
informed: []
upstream: []
---

# Provider and Model Identifiers

## Context and Problem Statement

Qualifier directories use provider and model names as path segments (`claude/claude-opus-4-6/MyRule.md`). These names must match the identifiers each provider uses in their API and documentation. Typos in directory names silently fail — a rule in `cladue/` is never resolved. The valid identifier set changes as providers release new models.

## Decision Drivers

- Directory names must match provider API model identifiers exactly
- Typos must be caught during assembly validation, not silently ignored
- New models are released regularly — the valid set must be updatable without code changes
- Each provider publishes their model identifiers in their API docs

## Considered Options

1. **Hardcoded identifiers** — valid names compiled into the binary. Requires recompilation for new models.
2. **Configurable models.yaml** — external config listing valid identifiers. Updatable without code changes.

## Decision Outcome

Maintain a `models.yaml` config file listing valid provider and model identifiers. Assembly validation checks qualifier directory names against this file and rejects unknown identifiers.

```yaml
# models.yaml — valid provider and model identifiers
# Update when providers release new models.
# Sources listed per provider.

claude:                         # https://docs.anthropic.com/en/docs/about-claude/models
    - claude-opus-4-6
    - claude-sonnet-4-6
    - claude-haiku-4-5

codex:                          # https://developers.openai.com/docs/models
    - o4-mini
    - o3
    - gpt-4.1

gemini:                         # https://ai.google.dev/gemini-api/docs/models
    - gemini-2.5-pro
    - gemini-2.5-flash
    - gemini-2.0-flash

opencode:                       # uses provider models, no own identifiers
    - claude-sonnet-4-6
    - claude-opus-4-6
```

### Validation

During `forge assemble` and `forge validate`, every qualifier directory name is checked against `models.yaml`:
- Top-level qualifiers must match a provider key or `user`
- Nested qualifiers must match a model identifier for that provider
- Unknown names produce an error with the closest match suggestion

### Updating

When a provider releases a new model, add it to `models.yaml`. No code change, no recompilation. The config ships with forge-cli and can be overridden via the standard `config.yaml` deep merge.

## Consequences

- [+] Typos in qualifier directories caught at assembly time
- [+] New models are a config update, not a code change
- [+] Source URLs documented per provider for easy verification
- [-] Must be kept in sync with provider releases

## More Information

[1]: https://docs.anthropic.com/en/docs/about-claude/models "Anthropic model identifiers"
[2]: https://developers.openai.com/docs/models "OpenAI model identifiers"
[3]: https://ai.google.dev/gemini-api/docs/models "Google Gemini model identifiers"
