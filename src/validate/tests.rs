use super::*;

const AGENT_BASIC: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/input/agent-basic.md"
));

const AGENT_INVALID: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/input/agent-invalid.md"
));

const AGENT_SCHEMA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/schemas/agent.schema.yaml"
));

const ADR_MDSCHEMA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/docs/decisions/.mdschema"
));

// --- validate (name pattern) ---

#[test]
fn valid_pascal_case_name() {
    let pattern = r"^[A-Z][a-zA-Z0-9]{2,50}$";
    assert!(validate("SecurityArchitect", pattern).is_ok());
    assert!(validate("TestAgent", pattern).is_ok());
    assert!(validate("QaTester", pattern).is_ok());
}

#[test]
fn rejects_empty_name() {
    let pattern = r"^[A-Z][a-zA-Z0-9]{2,50}$";
    let err = validate("", pattern).unwrap_err();
    assert!(err.contains("must not be empty"));
}

#[test]
fn rejects_short_name() {
    let pattern = r"^[A-Z][a-zA-Z0-9]{2,50}$";
    let err = validate("QA", pattern).unwrap_err();
    assert!(err.contains("does not match"));
}

#[test]
fn rejects_lowercase_start() {
    let pattern = r"^[A-Z][a-zA-Z0-9]{2,50}$";
    assert!(validate("myAgent", pattern).is_err());
}

#[test]
fn rejects_kebab_case_against_pascal() {
    let pattern = r"^[A-Z][a-zA-Z0-9]{2,50}$";
    assert!(validate("my-agent", pattern).is_err());
}

#[test]
fn accepts_kebab_case_with_kebab_pattern() {
    let pattern = r"^[a-z][a-z0-9-]{1,49}$";
    assert!(validate("code-reviewer", pattern).is_ok());
    assert!(validate("CodeReviewer", pattern).is_err());
}

#[test]
fn invalid_regex_pattern_returns_error() {
    let err = validate("Test", "[invalid").unwrap_err();
    assert!(err.contains("invalid name pattern"));
}

// --- validate_frontmatter ---

#[test]
fn valid_agent_frontmatter() {
    let diagnostics = validate_frontmatter(AGENT_BASIC, AGENT_SCHEMA, "agent-basic.md");
    assert!(
        diagnostics.is_empty(),
        "expected no diagnostics, got: {diagnostics:?}"
    );
}

#[test]
fn missing_required_fields() {
    let diagnostics = validate_frontmatter(AGENT_INVALID, AGENT_SCHEMA, "agent-invalid.md");

    let missing_name = diagnostics.iter().any(|d| d.message.contains("'name'"));
    let missing_desc = diagnostics
        .iter()
        .any(|d| d.message.contains("'description'"));

    assert!(missing_name, "expected diagnostic for missing 'name'");
    assert!(
        missing_desc,
        "expected diagnostic for missing 'description'"
    );
}

#[test]
fn pattern_mismatch_produces_error() {
    let content = "---\nname: bad-name\ndescription: test\n---\n# Body\n";
    let diagnostics = validate_frontmatter(content, AGENT_SCHEMA, "bad.md");

    let pattern_error = diagnostics
        .iter()
        .any(|d| d.message.contains("does not match pattern"));

    assert!(pattern_error, "expected pattern mismatch diagnostic");
}

#[test]
fn no_frontmatter_produces_error() {
    let content = "# Just a heading\n\nNo frontmatter here.\n";
    let diagnostics = validate_frontmatter(content, AGENT_SCHEMA, "no-fm.md");

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("missing frontmatter"));
    assert_eq!(diagnostics[0].severity, Severity::Error);
}

#[test]
fn schema_without_required_produces_no_diagnostics() {
    let content = "---\nfoo: bar\n---\n# Body\n";
    let schema = "properties:\n    foo:\n        type: string\n";
    let diagnostics = validate_frontmatter(content, schema, "test.md");
    assert!(diagnostics.is_empty());
}

// --- mdschema::check ---

#[test]
fn heading_skip_detected() {
    let content = "---\nstatus: Draft\n---\n# Title\n\n### Skipped h2\n";
    let schema = "heading_rules:\n    no_skip_levels: true\n    max_depth: 3\n";
    let diagnostics = mdschema::check(content, "skip.md", schema);

    let skip_error = diagnostics
        .iter()
        .any(|d| d.message.contains("skips from h1 to h3"));

    assert!(
        skip_error,
        "expected heading skip diagnostic, got: {diagnostics:?}"
    );
}

#[test]
fn heading_exceeds_max_depth() {
    let content = "---\ntitle: test\n---\n# H1\n\n## H2\n\n### H3\n\n#### H4 too deep\n";
    let schema = "heading_rules:\n    max_depth: 3\n";
    let diagnostics = mdschema::check(content, "deep.md", schema);

    let depth_error = diagnostics
        .iter()
        .any(|d| d.message.contains("exceeds max_depth"));

    assert!(
        depth_error,
        "expected max_depth diagnostic, got: {diagnostics:?}"
    );
}

#[test]
fn valid_headings_pass() {
    let content = "---\nstatus: ok\n---\n# Title\n\n## Section\n\n### Sub\n";
    let schema = "heading_rules:\n    no_skip_levels: true\n    max_depth: 3\n";
    let diagnostics = mdschema::check(content, "good.md", schema);
    assert!(
        diagnostics.is_empty(),
        "expected no diagnostics, got: {diagnostics:?}"
    );
}

#[test]
fn mdschema_missing_required_frontmatter() {
    let content = "---\ntitle: test\n---\n# Heading\n";
    let schema = "frontmatter:\n    fields:\n        - name: status\n          type: string\n        - name: title\n          type: string\n";
    let diagnostics = mdschema::check(content, "test.md", schema);

    let missing = diagnostics.iter().any(|d| d.message.contains("'status'"));

    assert!(missing, "expected missing 'status' diagnostic");
}

#[test]
fn mdschema_optional_fields_not_required() {
    let content = "---\nstatus: Draft\n---\n# Heading\n";
    let schema = "frontmatter:\n    fields:\n        - name: status\n          type: string\n        - name: tags\n          type: array\n          optional: true\n";
    let diagnostics = mdschema::check(content, "test.md", schema);
    assert!(
        diagnostics.is_empty(),
        "optional fields should not produce diagnostics"
    );
}

#[test]
fn mdschema_required_section_missing() {
    let content = "---\nstatus: Draft\n---\n# Title\n\n## Wrong Section\n";
    let schema = "structure:\n    - heading:\n          pattern: \"## Required Section\"\n";
    let diagnostics = mdschema::check(content, "test.md", schema);

    let section_error = diagnostics
        .iter()
        .any(|d| d.message.contains("missing required section"));

    assert!(section_error, "expected missing section diagnostic");
}

#[test]
fn mdschema_required_section_present() {
    let content = "---\nstatus: Draft\n---\n# Title\n\n## Required Section\n";
    let schema = "structure:\n    - heading:\n          pattern: \"## Required Section\"\n";
    let diagnostics = mdschema::check(content, "test.md", schema);
    assert!(diagnostics.is_empty());
}

#[test]
fn mdschema_regex_section_matching() {
    let content =
        "---\nstatus: Draft\n---\n# My Custom Title\n\n## Context and Problem Statement\n";
    let schema =
        "structure:\n    - heading:\n          pattern: \"^# .+\"\n          regex: true\n";
    let diagnostics = mdschema::check(content, "test.md", schema);
    assert!(diagnostics.is_empty());
}

#[test]
fn mdschema_invalid_schema_yaml() {
    let diagnostics = mdschema::check("# Test\n", "test.md", "invalid: yaml: [");

    let parse_error = diagnostics
        .iter()
        .any(|d| d.message.contains("invalid mdschema"));

    assert!(parse_error, "expected parse error diagnostic");
}

#[test]
fn adr_mdschema_catches_missing_section() {
    let adr = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/input/adr-missing-section.md"
    ));
    let diagnostics = mdschema::check(adr, "adr-missing-section.md", ADR_MDSCHEMA);

    let missing_section = diagnostics
        .iter()
        .any(|d| d.message.contains("Considered Options"));

    assert!(
        missing_section,
        "expected missing 'Considered Options' diagnostic"
    );
}

// --- plugin scaffolding validators (schema-light) ---

#[test]
fn json_manifest_valid_passes() {
    let content = r#"{"name": "my-plugin", "anything": ["here"]}"#;
    let diagnostics = validate_json_manifest(content, ".claude-plugin/plugin.json");
    assert!(diagnostics.is_empty(), "valid JSON: {diagnostics:?}");
}

#[test]
fn json_manifest_invalid_errors() {
    let diagnostics = validate_json_manifest("{not json", "plugin.json");
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("invalid JSON"));
    assert_eq!(diagnostics[0].severity, Severity::Error);
}

#[test]
fn json_manifest_does_not_assert_field_shape() {
    // No name, non-kebab fields, arbitrary shape: as long as it parses, it passes.
    // The schema is Claude Code's to define, not ours to police.
    let content = r#"{"OWNER": "Whatever", "plugins": []}"#;
    let diagnostics = validate_json_manifest(content, "marketplace.json");
    assert!(
        diagnostics.is_empty(),
        "shape is not asserted: {diagnostics:?}"
    );
}

#[test]
fn hooks_manifest_invalid_json_errors() {
    let (diagnostics, scripts) = validate_hooks_manifest("{bad", "hooks.json");
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("invalid JSON"));
    assert!(scripts.is_empty());
}

#[test]
fn hooks_manifest_extracts_plugin_root_script() {
    let content = r#"{
        "hooks": {
            "PreToolUse": [
                {"hooks": [{"type": "command", "command": "${CLAUDE_PLUGIN_ROOT}/hooks/guard.sh --flag"}]}
            ]
        }
    }"#;
    let (diagnostics, scripts) = validate_hooks_manifest(content, "hooks.json");
    assert!(diagnostics.is_empty(), "valid hooks JSON: {diagnostics:?}");
    assert_eq!(scripts, vec!["hooks/guard.sh".to_string()]);
}

#[test]
fn hooks_manifest_ignores_non_convention_command() {
    // A command that does not use ${CLAUDE_PLUGIN_ROOT} is not an error; it
    // simply yields no script to check.
    let content = r#"{
        "hooks": {
            "PreToolUse": [
                {"hooks": [{"type": "command", "command": "/usr/bin/foo.sh"}]}
            ]
        }
    }"#;
    let (diagnostics, scripts) = validate_hooks_manifest(content, "hooks.json");
    assert!(
        diagnostics.is_empty(),
        "non-convention command is not flagged"
    );
    assert!(scripts.is_empty());
}
