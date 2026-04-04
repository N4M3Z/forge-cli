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

const SCALARS: &str = fixture!("yaml-scalars.yaml");
const DOTTED_KEYS: &str = fixture!("yaml-dotted-keys.yaml");
const NESTED: &str = fixture!("yaml-nested.yaml");
const LISTS: &str = fixture!("yaml-lists.yaml");

// --- yaml_value: scalar extraction ---

#[test]
fn value_extracts_simple_string() {
    assert_eq!(yaml_value(SCALARS, "title"), Some("Hello World".into()));
}

#[test]
fn value_extracts_quoted_string() {
    assert_eq!(
        yaml_value(SCALARS, "description"),
        Some("A quoted value".into())
    );
}

#[test]
fn value_extracts_url_with_colon() {
    assert_eq!(
        yaml_value(SCALARS, "url"),
        Some("https://example.com".into())
    );
}

#[test]
fn value_extracts_boolean_as_string() {
    assert_eq!(yaml_value(SCALARS, "draft"), Some("true".into()));
}

#[test]
fn value_extracts_number_as_string() {
    assert_eq!(yaml_value(SCALARS, "priority"), Some("42".into()));
}

#[test]
fn value_returns_none_for_null() {
    assert_eq!(yaml_value(SCALARS, "empty"), None);
}

#[test]
fn value_returns_none_for_missing_key() {
    assert_eq!(yaml_value(SCALARS, "nonexistent"), None);
}

// --- yaml_value: dotted key resolution ---

#[test]
fn value_resolves_flat_dotted_key() {
    assert_eq!(
        yaml_value(DOTTED_KEYS, "claude.name"),
        Some("SecurityArchitect".into())
    );
}

#[test]
fn value_resolves_flat_dotted_key_model() {
    assert_eq!(
        yaml_value(DOTTED_KEYS, "claude.model"),
        Some("sonnet".into())
    );
}

#[test]
fn value_does_not_match_similar_flat_key() {
    assert_eq!(yaml_value(DOTTED_KEYS, "claudeXname"), Some("Wrong".into()));
    assert_eq!(
        yaml_value(DOTTED_KEYS, "claude.name"),
        Some("SecurityArchitect".into())
    );
}

// --- yaml_value: nested access ---

#[test]
fn value_resolves_nested_key() {
    assert_eq!(yaml_value(NESTED, "user.root"), Some("/Users/alice".into()));
}

#[test]
fn value_resolves_deeply_nested_key() {
    assert_eq!(
        yaml_value(NESTED, "user.settings.theme"),
        Some("dark".into())
    );
}

#[test]
fn value_returns_none_for_partial_nested_path() {
    assert_eq!(yaml_value(NESTED, "user.settings.missing"), None);
}

// --- yaml_list: sequence extraction ---

#[test]
fn list_extracts_yaml_sequence() {
    assert_eq!(yaml_list(LISTS, "tags"), Some("one, two, three".into()));
}

#[test]
fn list_returns_string_as_is() {
    assert_eq!(yaml_list(LISTS, "tools"), Some("Read, Write, Bash".into()));
}

#[test]
fn list_returns_none_for_empty_sequence() {
    assert_eq!(yaml_list(LISTS, "empty_list"), None);
}

#[test]
fn list_resolves_dotted_sequence() {
    assert_eq!(yaml_list(LISTS, "claude.tools"), Some("Read, Write".into()));
}

#[test]
fn list_handles_mixed_scalar_types() {
    assert_eq!(yaml_list(LISTS, "mixed"), Some("hello, 42, true".into()));
}

#[test]
fn list_returns_none_for_missing_key() {
    assert_eq!(yaml_list(LISTS, "nonexistent"), None);
}

// --- edge cases ---

#[test]
fn value_returns_none_for_empty_input() {
    assert_eq!(yaml_value("", "key"), None);
}

#[test]
fn list_returns_none_for_empty_input() {
    assert_eq!(yaml_list("", "key"), None);
}

// --- deep_merge ---

const MERGE_DEFAULTS: &str = fixture!("yaml-merge-defaults.yaml");
const MERGE_OVERRIDE: &str = fixture!("yaml-merge-override.yaml");

#[test]
fn merge_override_replaces_scalar() {
    let merged = deep_merge(MERGE_DEFAULTS, MERGE_OVERRIDE).unwrap();
    assert_eq!(yaml_value(&merged, "user.root"), Some("/custom".into()));
}

#[test]
fn merge_preserves_unoverridden_nested_key() {
    let merged = deep_merge(MERGE_DEFAULTS, MERGE_OVERRIDE).unwrap();
    assert_eq!(
        yaml_value(&merged, "user.settings.font_size"),
        Some("14".into())
    );
}

#[test]
fn merge_overrides_nested_scalar() {
    let merged = deep_merge(MERGE_DEFAULTS, MERGE_OVERRIDE).unwrap();
    assert_eq!(
        yaml_value(&merged, "user.settings.theme"),
        Some("dark".into())
    );
}

#[test]
fn merge_preserves_top_level_default() {
    let merged = deep_merge(MERGE_DEFAULTS, MERGE_OVERRIDE).unwrap();
    assert_eq!(yaml_value(&merged, "debug"), Some("false".into()));
}

#[test]
fn merge_adds_new_top_level_key() {
    let merged = deep_merge(MERGE_DEFAULTS, MERGE_OVERRIDE).unwrap();
    assert_eq!(yaml_value(&merged, "extra"), Some("true".into()));
}

#[test]
fn merge_sequence_replaced_by_override() {
    let override_with_seq = "tags:\n    - alpha\n";
    let merged = deep_merge(MERGE_DEFAULTS, override_with_seq).unwrap();
    assert_eq!(yaml_list(&merged, "tags"), Some("alpha".into()));
}

#[test]
fn merge_rejects_invalid_defaults() {
    let result = deep_merge("not: valid: yaml: {{", "key: value");
    assert!(result.is_err());
}

#[test]
fn merge_rejects_invalid_override() {
    let result = deep_merge("key: value", "not: valid: yaml: {{");
    assert!(result.is_err());
}

#[test]
fn merge_both_empty_mappings() {
    let merged = deep_merge("{}", "{}").unwrap();
    assert_eq!(yaml_value(&merged, "anything"), None);
}

#[test]
fn merge_keeps_default_when_override_has_sequence_for_mapping() {
    let defaults = "models:\n    claude:\n        strong: opus\n";
    let override_with_sequence = "models:\n    - opus\n    - sonnet\n";
    let merged = deep_merge(defaults, override_with_sequence).unwrap();
    assert_eq!(
        yaml_value(&merged, "models.claude.strong"),
        Some("opus".into())
    );
}

#[test]
fn merge_keeps_default_when_override_has_mapping_for_scalar() {
    let defaults = "version: 1\n";
    let override_with_mapping = "version:\n    major: 2\n";
    let merged = deep_merge(defaults, override_with_mapping).unwrap();
    assert_eq!(yaml_value(&merged, "version"), Some("1".into()));
}

#[test]
fn merge_keeps_default_when_nested_type_conflicts() {
    let defaults = "providers:\n    claude:\n        models:\n            strong: opus\n";
    let override_content = "providers:\n    claude:\n        models:\n            - opus\n";
    let merged = deep_merge(defaults, override_content).unwrap();
    assert_eq!(
        yaml_value(&merged, "providers.claude.models.strong"),
        Some("opus".into())
    );
}
