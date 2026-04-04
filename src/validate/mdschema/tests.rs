use super::heading::extract_headings;

#[test]
fn extract_headings_finds_all_levels() {
    let body = "# Title\n\n## Section\n\n### Subsection\n";
    let headings = extract_headings(body);
    assert_eq!(headings.len(), 3);
    assert_eq!(headings[0].level, 1);
    assert_eq!(headings[1].level, 2);
    assert_eq!(headings[2].level, 3);
}

#[test]
fn extract_headings_skips_code_fences() {
    let body = "# Real\n\n```\n# Not a heading\n```\n\n## Also Real\n";
    let headings = extract_headings(body);
    assert_eq!(headings.len(), 2);
    assert_eq!(headings[0].text, "Real");
    assert_eq!(headings[1].text, "Also Real");
}

#[test]
fn extract_headings_empty_body() {
    let headings = extract_headings("");
    assert!(headings.is_empty());
}

#[test]
fn extract_headings_preserves_text() {
    let body = "## Context and Problem Statement\n";
    let headings = extract_headings(body);
    assert_eq!(headings[0].text, "Context and Problem Statement");
}

#[test]
fn check_invalid_schema_returns_error() {
    let diagnostics = super::check("# Doc\n", "test.md", "not: [valid: yaml");
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("invalid mdschema"));
}

#[test]
fn check_empty_schema_produces_no_diagnostics() {
    let diagnostics = super::check("# Doc\n", "test.md", "{}");
    assert!(diagnostics.is_empty());
}

#[test]
fn check_optional_frontmatter_not_required() {
    let schema = "frontmatter:\n    fields:\n        - name: tags\n          type: array\n          optional: true\n";
    let content = "---\ntitle: test\n---\n# Doc\n";
    let diagnostics = super::check(content, "test.md", schema);
    assert!(diagnostics.is_empty());
}
