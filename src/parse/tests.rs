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

const AGENT_BASIC: &str = fixture!("agent-basic.md");
const SKILL_BASIC: &str = fixture!("skill-basic.md");
const NO_FRONTMATTER: &str = fixture!("no-frontmatter.md");
const EXPECTED_BODY: &str = expected!("agent-basic-body.md");

// --- split_frontmatter ---

#[test]
fn split_returns_yaml_and_body() {
    let (yaml_text, body) = split_frontmatter(AGENT_BASIC).unwrap();
    assert!(yaml_text.contains("name: TestAgent"));
    assert!(yaml_text.contains("version: 0.1.0"));
    assert!(body.contains("# TestAgent"));
}

#[test]
fn split_returns_none_without_frontmatter() {
    assert!(split_frontmatter(NO_FRONTMATTER).is_none());
}

#[test]
fn split_handles_empty_frontmatter() {
    let content = "---\n---\nBody";
    let (yaml_text, body) = split_frontmatter(content).unwrap();
    assert_eq!(yaml_text, "");
    assert_eq!(body, "Body");
}

#[test]
fn split_returns_none_for_unclosed_frontmatter() {
    let content = "---\ntitle: Hello\nno closing delimiter";
    assert!(split_frontmatter(content).is_none());
}

#[test]
fn split_rejects_oversized_content() {
    let oversized = format!("---\ntitle: x\n---\n{}", "x".repeat(256 * 1024));
    assert!(split_frontmatter(&oversized).is_none());
}

// --- frontmatter_value ---

#[test]
fn value_extracts_name_from_agent() {
    assert_eq!(
        frontmatter_value(AGENT_BASIC, "name"),
        Some("TestAgent".into())
    );
}

#[test]
fn value_extracts_version_from_agent() {
    assert_eq!(
        frontmatter_value(AGENT_BASIC, "version"),
        Some("0.1.0".into())
    );
}

#[test]
fn value_extracts_model_from_agent() {
    assert_eq!(frontmatter_value(AGENT_BASIC, "model"), Some("fast".into()));
}

#[test]
fn value_extracts_name_from_skill() {
    assert_eq!(
        frontmatter_value(SKILL_BASIC, "name"),
        Some("ExampleSkill".into())
    );
}

#[test]
fn value_returns_none_for_missing_key() {
    assert_eq!(frontmatter_value(AGENT_BASIC, "nonexistent"), None);
}

#[test]
fn value_returns_none_without_frontmatter() {
    assert_eq!(frontmatter_value(NO_FRONTMATTER, "name"), None);
}

// --- frontmatter_body ---

#[test]
fn body_contains_expected_content() {
    let body = frontmatter_body(AGENT_BASIC);
    assert!(body.contains(EXPECTED_BODY.trim()));
}

#[test]
fn body_returns_full_content_without_frontmatter() {
    let body = frontmatter_body(NO_FRONTMATTER);
    assert_eq!(body, NO_FRONTMATTER);
}

#[test]
fn body_preserves_leading_blank_line() {
    let body = frontmatter_body(AGENT_BASIC);
    assert!(body.starts_with('\n'));
}

// --- frontmatter_list ---

#[test]
fn list_extracts_tools_as_string() {
    assert_eq!(
        frontmatter_list(AGENT_BASIC, "tools"),
        Some("Read, Grep".into())
    );
}

#[test]
fn list_returns_none_for_missing_key() {
    assert_eq!(frontmatter_list(SKILL_BASIC, "tools"), None);
}

#[test]
fn list_returns_none_without_frontmatter() {
    assert_eq!(frontmatter_list(NO_FRONTMATTER, "tools"), None);
}
