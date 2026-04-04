---
status: Proposed
date: 2026-03-19
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["WebResearcher"]
informed: []
tags: [assembly, validation, json-schema]
---

# Validation via YAML Schema

## Context and Problem Statement

Module validation hardcodes frontmatter field requirements, naming patterns, and content markers as Rust match arms. Frontmatter validation is a solved problem — JSON Schema is the standard for defining "this YAML must have these fields with these types." Tools like `check-jsonschema`, `yq`, and `ajv-cli` already validate YAML against JSON Schema. No custom validation code is needed for frontmatter.

## Decision Drivers

- JSON Schema is a universal standard with tooling in every language
- `yq` extracts YAML frontmatter from markdown natively
- `forge yaml` ships with forge-cli and can serve as a fallback when `yq` is unavailable
- Skills should validate against the upstream Agent Skills spec
- Validation rules should be editable without recompilation

## Decision Outcome

Ship JSON Schema files per content type, authored as YAML (YAML is a superset of JSON — tools like `check-jsonschema` accept both). The spec is called "JSON Schema" but the schema files are `.schema.yaml` for consistency with the rest of the ecosystem.

### Schema files

```
schemas/
    agent.schema.yaml
    skill.schema.yaml
    rule.schema.yaml
    module.schema.yaml
```

Example `agent.schema.yaml`:

```yaml
$schema: https://json-schema.org/draft/2020-12/schema
type: object
required: [name, description]
properties:
    name:
        type: string
        pattern: "^[A-Z][a-zA-Z0-9]{2,50}$"
    description:
        type: string
        pattern: "USE WHEN"
    version:
        type: string
```

### Validation chain

```
forge validate .
    │
    ├── frontmatter:  forge yaml / yq  →  check-jsonschema --schemafile schemas/agent.schema.json
    ├── structure:    markdownlint / mdschema
    ├── naming:       ls-lint / shell glob
    └── content:      DCI regex checks (custom, minimal)
```

Tool precedence: prefer external tools when installed (`yq`, `check-jsonschema`, `markdownlint`, `ls-lint`). Fall back to `forge yaml` for extraction and minimal built-in checks when external tools are unavailable.

### What validates where

| Content | Schema                   | Tool                            |
| ------- | ------------------------ | ------------------------------- |
| Skills  | Agent Skills spec        | `skill-validator` or schema     |
| Plugins | Claude Code plugin spec  | `claude plugin validate`        |
| Agents  | `agent.schema.json`      | `check-jsonschema` / built-in   |
| Rules   | `rule.schema.json`       | `check-jsonschema` / built-in   |
| Module  | `module.schema.json`     | `check-jsonschema` / built-in   |
| DCI     | regex patterns in config | built-in (no external tool)     |

### Consequences

- [+] JSON Schema is universal — anyone can read, edit, validate with any tool
- [+] No custom validation code for frontmatter
- [+] Schema files are self-documenting
- [+] Works with or without external tools installed
- [-] JSON Schema cannot express cross-field constraints easily (e.g., "name must match filename")
- [-] DCI and heading validation still need minimal custom code
