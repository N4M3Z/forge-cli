use super::*;

macro_rules! fixture {
    ($name:expr) => {
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/input/",
            $name
        ))
    };
}

macro_rules! expected {
    ($name:expr) => {
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/expected/",
            $name
        ))
    };
}

const RULE_WITH_REFS: &str = fixture!("rule-with-refs.md");
const EXPECTED_STRIPPED: &str = expected!("rule-stripped.md");
const EXPECTED_APPEND: &str = expected!("variant-append-result.md");
const EXPECTED_PREPEND: &str = expected!("variant-prepend-result.md");
const EXPECTED_KEPT: &str = expected!("frontmatter-kept.md");
const VARIANT_APPEND: &str = fixture!("variant-append.md");
const VARIANT_PREPEND: &str = fixture!("variant-prepend.md");
const VARIANT_REPLACE: &str = fixture!("variant-replace.md");
const AGENT_BASIC: &str = fixture!("agent-basic.md");
const FRONTMATTER_SIMPLE: &str = fixture!("frontmatter-simple.md");
const NO_FRONTMATTER_BODY: &str = fixture!("no-frontmatter-body.md");
const RULE_BASE: &str = fixture!("rule-base.md");
const REFS_DEFINITION_ONLY: &str = fixture!("refs-definition-only.md");
const REFS_INLINE_AND_DEF: &str = fixture!("refs-inline-and-def.md");
const PLAIN_NO_REFS: &str = fixture!("plain-no-refs.md");

// --- strip_frontmatter ---

#[test]
fn strip_frontmatter_removes_all_when_no_keep_fields() {
    let result = strip_frontmatter(AGENT_BASIC, &[]);
    assert!(!result.contains("---"));
    assert!(!result.contains("name:"));
    assert!(result.contains("This is a test agent"));
}

#[test]
fn strip_frontmatter_keeps_specified_fields() {
    let result = strip_frontmatter(AGENT_BASIC, &["name", "model"]);
    assert!(result.contains("---"));
    assert!(result.contains("name: TestAgent"));
    assert!(result.contains("model: fast"));
    assert!(!result.contains("version:"));
    assert!(!result.contains("description:"));
}

#[test]
fn strip_frontmatter_strips_heading() {
    let result = strip_frontmatter(AGENT_BASIC, &[]);
    assert!(!result.contains("# TestAgent"));
}

#[test]
fn strip_frontmatter_returns_body_without_frontmatter() {
    let result = strip_frontmatter(NO_FRONTMATTER_BODY, &[]);
    assert_eq!(result.trim(), NO_FRONTMATTER_BODY.trim());
}

#[test]
fn strip_frontmatter_empty_keep_fields_strips_fences() {
    let result = strip_frontmatter(FRONTMATTER_SIMPLE, &[]);
    assert!(!result.contains("---"));
    assert!(result.contains("Body text."));
}

#[test]
fn strip_frontmatter_unmatched_keep_fields_strips_all() {
    let result = strip_frontmatter(AGENT_BASIC, &["nonexistent"]);
    assert!(!result.contains("---"));
}

#[test]
fn strip_frontmatter_keeps_specified_fields_case_insensitively() {
    let content = "---\nName: TestAgent\nVERSION: 0.1.0\n---";
    let result = strip_frontmatter(content, &["name"]);
    assert!(result.contains("Name: TestAgent"));
    assert!(!result.contains("VERSION:"));
}

#[test]
fn map_field_finds_name_after_other_fields() {
    let content = "---\ndescription: test\nname: TestAgent\n---";
    let result = map_field(content, "name", |v| v.to_lowercase());
    assert!(result.contains("name: testagent"));
    assert!(result.contains("description: test"));
}

#[test]
fn map_field_handles_double_quoted_value() {
    let content = "---\nname: \"SecurityArchitect\"\n---";
    let result = map_field(content, "name", |v| v.to_lowercase());
    assert!(
        result.contains("name: securityarchitect"),
        "quoted value should be unwrapped before mapping: {result}"
    );
}

#[test]
fn map_field_handles_single_quoted_value() {
    let content = "---\nname: 'SecurityArchitect'\n---";
    let result = map_field(content, "name", |v| v.to_lowercase());
    assert!(
        result.contains("name: securityarchitect"),
        "single-quoted value should be unwrapped before mapping: {result}"
    );
}

#[test]
fn map_field_returns_unchanged_when_field_missing() {
    let content = "---\ndescription: test\n---\nBody.";
    let result = map_field(content, "name", |v| v.to_lowercase());
    assert_eq!(result, content);
}

// --- references::strip ---

#[test]
fn strip_removes_inline_markers() {
    let result = references::strip("Text with a ref [1] and another [2].");
    assert_eq!(result, "Text with a ref and another.");
}

#[test]
fn strip_removes_definition_lines() {
    let result = references::strip(REFS_DEFINITION_ONLY);
    assert_eq!(result, "Body text.\n");
}

#[test]
fn strip_removes_both_markers_and_definitions() {
    let result = references::strip(REFS_INLINE_AND_DEF);
    assert_eq!(result, "Paragraph here.\n");
}

#[test]
fn strip_preserves_content_without_refs() {
    let result = references::strip(PLAIN_NO_REFS);
    assert_eq!(result, PLAIN_NO_REFS);
}

// --- references::extract ---

#[test]
fn extract_returns_urls() {
    let urls = references::extract(RULE_WITH_REFS);
    assert_eq!(urls.len(), 2);
    assert_eq!(urls[0], "https://example.com/source-one");
    assert_eq!(urls[1], "https://example.com/source-two");
}

#[test]
fn extract_returns_empty_for_no_refs() {
    let urls = references::extract("No refs here.");
    assert!(urls.is_empty());
}

const REFS_MNEMONIC: &str = fixture!("refs-mnemonic.md");

#[test]
fn strip_removes_mnemonic_inline_markers() {
    let result = references::strip("Text with a ref [MADR] and another [OWASP].");
    assert_eq!(result, "Text with a ref and another.");
}

#[test]
fn strip_removes_mnemonic_definitions() {
    let result = references::strip(REFS_MNEMONIC);
    assert!(!result.contains("[MADR]"));
    assert!(!result.contains("[OWASP]"));
    assert!(!result.contains("[keepachangelog]"));
    assert!(!result.contains("https://adr.github.io"));
}

#[test]
fn extract_returns_mnemonic_urls() {
    let urls = references::extract(REFS_MNEMONIC);
    assert_eq!(urls.len(), 3);
    assert_eq!(urls[0], "https://adr.github.io/madr/");
    assert_eq!(urls[1], "https://owasp.org/");
    assert_eq!(urls[2], "https://keepachangelog.com/");
}

// --- variants::Mode ---

#[test]
fn mode_parses_append() {
    assert_eq!(variants::Mode::parse("append"), variants::Mode::Append);
}

#[test]
fn mode_parses_prepend() {
    assert_eq!(variants::Mode::parse("prepend"), variants::Mode::Prepend);
}

#[test]
fn mode_defaults_to_replace() {
    assert_eq!(variants::Mode::parse("unknown"), variants::Mode::Replace);
    assert_eq!(variants::Mode::parse(""), variants::Mode::Replace);
}

// --- variants::apply (golden output) ---

#[test]
fn apply_append_matches_golden_output() {
    let result = variants::apply(RULE_BASE, VARIANT_APPEND, variants::Mode::Append);
    assert_eq!(result.trim(), EXPECTED_APPEND.trim());
}

#[test]
fn apply_prepend_matches_golden_output() {
    let result = variants::apply(RULE_BASE, VARIANT_PREPEND, variants::Mode::Prepend);
    assert_eq!(result.trim(), EXPECTED_PREPEND.trim());
}

#[test]
fn apply_replaces_with_variant_body() {
    let result = variants::apply(RULE_BASE, VARIANT_REPLACE, variants::Mode::Replace);
    assert!(!result.contains("Base body."));
    assert!(result.contains("Replacement body."));
}

// --- strip_frontmatter (golden output) ---

#[test]
fn strip_frontmatter_keep_name_matches_golden_output() {
    let result = strip_frontmatter(AGENT_BASIC, &["name"]);
    assert_eq!(result.trim(), EXPECTED_KEPT.trim());
}

// --- variants::resolve ---

#[test]
fn resolve_returns_none_for_missing_files() {
    let dir = std::path::Path::new("/nonexistent/path");
    let qualifiers = vec!["user".to_string(), "anthropic".to_string()];
    let result = variants::resolve(dir, "rule.md", &qualifiers);
    assert!(result.is_none());
}

#[test]
fn resolve_finds_user_variant() {
    let dir = tempfile::tempdir().unwrap();
    let user_dir = dir.path().join("user");
    std::fs::create_dir(&user_dir).unwrap();
    std::fs::write(user_dir.join("rule.md"), "user variant").unwrap();

    let qualifiers = vec!["user".to_string(), "anthropic".to_string()];
    let result = variants::resolve(dir.path(), "rule.md", &qualifiers);

    assert_eq!(result.unwrap(), user_dir.join("rule.md"));
}

#[test]
fn resolve_finds_provider_variant() {
    let dir = tempfile::tempdir().unwrap();
    let provider_dir = dir.path().join("anthropic");
    std::fs::create_dir(&provider_dir).unwrap();
    std::fs::write(provider_dir.join("rule.md"), "provider variant").unwrap();

    let qualifiers = vec!["user".to_string(), "anthropic".to_string()];
    let result = variants::resolve(dir.path(), "rule.md", &qualifiers);

    assert_eq!(result.unwrap(), provider_dir.join("rule.md"));
}

#[test]
fn resolve_user_takes_precedence_over_provider() {
    let dir = tempfile::tempdir().unwrap();

    let user_dir = dir.path().join("user");
    std::fs::create_dir(&user_dir).unwrap();
    std::fs::write(user_dir.join("rule.md"), "user variant").unwrap();

    let provider_dir = dir.path().join("anthropic");
    std::fs::create_dir(&provider_dir).unwrap();
    std::fs::write(provider_dir.join("rule.md"), "provider variant").unwrap();

    let qualifiers = vec!["user".to_string(), "anthropic".to_string()];
    let result = variants::resolve(dir.path(), "rule.md", &qualifiers);

    assert_eq!(result.unwrap(), user_dir.join("rule.md"));
}

#[test]
fn resolve_provider_model_takes_precedence_over_provider() {
    let dir = tempfile::tempdir().unwrap();

    let provider_dir = dir.path().join("anthropic");
    std::fs::create_dir(&provider_dir).unwrap();
    std::fs::write(provider_dir.join("rule.md"), "provider variant").unwrap();

    let model_dir = provider_dir.join("sonnet");
    std::fs::create_dir(&model_dir).unwrap();
    std::fs::write(model_dir.join("rule.md"), "model variant").unwrap();

    let qualifiers = vec!["anthropic".to_string(), "sonnet".to_string()];
    let result = variants::resolve(dir.path(), "rule.md", &qualifiers);

    assert_eq!(result.unwrap(), model_dir.join("rule.md"));
}

// --- assemble (pipeline) ---

#[test]
fn assemble_strips_frontmatter_and_refs() {
    let result = assemble(RULE_WITH_REFS, None, &[], true);
    assert_eq!(result.trim(), EXPECTED_STRIPPED.trim());
}

#[test]
fn assemble_with_append_variant() {
    let result = assemble(RULE_WITH_REFS, Some(VARIANT_APPEND), &[], true);
    assert!(result.contains("First paragraph with a reference."));
    assert!(result.contains("This content is appended"));
}

#[test]
fn assemble_with_prepend_variant() {
    let result = assemble(RULE_WITH_REFS, Some(VARIANT_PREPEND), &[], true);
    assert!(result.contains("First paragraph with a reference."));
    assert!(result.contains("This content is prepended"));
    let prepend_pos = result.find("This content is prepended").unwrap();
    let body_pos = result.find("First paragraph").unwrap();
    assert!(prepend_pos < body_pos);
}

#[test]
fn assemble_keeps_specified_frontmatter_fields() {
    let result = assemble(AGENT_BASIC, None, &["name"], true);
    assert!(result.contains("---"));
    assert!(result.contains("name: TestAgent"));
    assert!(!result.contains("version:"));
}

#[test]
fn assemble_no_variant_no_keep_strips_everything() {
    let result = assemble(AGENT_BASIC, None, &[], true);
    assert!(!result.contains("---"));
    assert!(!result.contains("# TestAgent"));
    assert!(result.contains("This is a test agent"));
}
