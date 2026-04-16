use super::*;

const SCHEMA: &str = r#"{
    "type": "object",
    "required": ["title", "type", "status", "created", "tags"],
    "properties": {
        "title": {"type": "string", "minLength": 1, "maxLength": 100},
        "type": {"type": "string", "const": "adr"},
        "status": {"type": "string", "enum": ["proposed", "accepted", "deprecated", "superseded"]},
        "created": {"type": "string", "format": "date"},
        "tags": {
            "type": "array",
            "minItems": 1,
            "uniqueItems": true,
            "items": {"type": "string", "minLength": 1, "pattern": "^[a-z0-9][a-z0-9-]*[a-z0-9]$|^[a-z0-9]$"}
        }
    }
}"#;

fn frontmatter(yaml_body: &str) -> String {
    format!("---\n{yaml_body}\n---\nBody content.\n")
}

fn valid_frontmatter() -> String {
    frontmatter(
        "title: Test Decision\ntype: adr\nstatus: accepted\ncreated: \"2026-03-30\"\ntags:\n    - architecture",
    )
}

#[test]
fn valid_adr_passes() {
    let content = valid_frontmatter();
    let diagnostics = validate_frontmatter_against_json_schema(&content, SCHEMA, "test.md");
    assert!(diagnostics.is_empty(), "unexpected errors: {diagnostics:?}");
}

#[test]
fn missing_required_field_fails() {
    let content = frontmatter(
        "type: adr\nstatus: accepted\ncreated: \"2026-03-30\"\ntags:\n    - architecture",
    );
    let diagnostics = validate_frontmatter_against_json_schema(&content, SCHEMA, "test.md");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("title")),
        "expected title error: {diagnostics:?}"
    );
}

#[test]
fn invalid_enum_fails() {
    let content = frontmatter(
        "title: Test\ntype: adr\nstatus: fantasy\ncreated: \"2026-03-30\"\ntags:\n    - architecture",
    );
    let diagnostics = validate_frontmatter_against_json_schema(&content, SCHEMA, "test.md");
    assert!(!diagnostics.is_empty());
}

#[test]
fn invalid_const_fails() {
    let content = frontmatter(
        "title: Test\ntype: not-adr\nstatus: accepted\ncreated: \"2026-03-30\"\ntags:\n    - architecture",
    );
    let diagnostics = validate_frontmatter_against_json_schema(&content, SCHEMA, "test.md");
    assert!(!diagnostics.is_empty());
}

#[test]
fn invalid_date_format_fails() {
    let content = frontmatter(
        "title: Test\ntype: adr\nstatus: accepted\ncreated: March 30 2026\ntags:\n    - architecture",
    );
    let diagnostics = validate_frontmatter_against_json_schema(&content, SCHEMA, "test.md");
    assert!(!diagnostics.is_empty());
}

#[test]
fn invalid_tag_pattern_fails() {
    let content = frontmatter(
        "title: Test\ntype: adr\nstatus: accepted\ncreated: \"2026-03-30\"\ntags:\n    - UPPER_CASE",
    );
    let diagnostics = validate_frontmatter_against_json_schema(&content, SCHEMA, "test.md");
    assert!(!diagnostics.is_empty());
}

#[test]
fn tags_as_string_fails() {
    let content = frontmatter(
        "title: Test\ntype: adr\nstatus: accepted\ncreated: \"2026-03-30\"\ntags: not-an-array",
    );
    let diagnostics = validate_frontmatter_against_json_schema(&content, SCHEMA, "test.md");
    assert!(!diagnostics.is_empty());
}

#[test]
fn empty_tags_fails() {
    let content =
        frontmatter("title: Test\ntype: adr\nstatus: accepted\ncreated: \"2026-03-30\"\ntags: []");
    let diagnostics = validate_frontmatter_against_json_schema(&content, SCHEMA, "test.md");
    assert!(!diagnostics.is_empty());
}

#[test]
fn empty_title_fails() {
    let content = frontmatter(
        "title: \"\"\ntype: adr\nstatus: accepted\ncreated: \"2026-03-30\"\ntags:\n    - architecture",
    );
    let diagnostics = validate_frontmatter_against_json_schema(&content, SCHEMA, "test.md");
    assert!(!diagnostics.is_empty());
}

#[test]
fn duplicate_tags_fails() {
    let content = frontmatter(
        "title: Test\ntype: adr\nstatus: accepted\ncreated: \"2026-03-30\"\ntags:\n    - architecture\n    - architecture",
    );
    let diagnostics = validate_frontmatter_against_json_schema(&content, SCHEMA, "test.md");
    assert!(!diagnostics.is_empty());
}

#[test]
fn overlong_title_fails() {
    let long_title = "A".repeat(101);
    let content = frontmatter(&format!(
        "title: {long_title}\ntype: adr\nstatus: accepted\ncreated: \"2026-03-30\"\ntags:\n    - architecture"
    ));
    let diagnostics = validate_frontmatter_against_json_schema(&content, SCHEMA, "test.md");
    assert!(!diagnostics.is_empty());
}

#[test]
fn no_frontmatter_returns_empty() {
    let diagnostics =
        validate_frontmatter_against_json_schema("No frontmatter here.", SCHEMA, "test.md");
    assert!(diagnostics.is_empty());
}
