use std::collections::HashMap;

use super::*;
use crate::provider::AssemblyRule;

const REMAP_YAML: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/config/remap-tools.yaml",
));

const AGENT_FIXTURE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/input/agent-basic.md",
));

// --- to_kebab_case ---

#[test]
fn kebab_case_converts_pascal_case() {
    assert_eq!(to_kebab_case("SecurityArchitect"), "security-architect");
}

#[test]
fn kebab_case_handles_consecutive_uppercase() {
    assert_eq!(to_kebab_case("XMLParser"), "xml-parser");
    assert_eq!(to_kebab_case("QATester"), "qa-tester");
}

#[test]
fn kebab_case_keeps_abbreviation_bridges_together() {
    assert_eq!(to_kebab_case("DnDBeyondHomebrew"), "dnd-beyond-homebrew");
}

#[test]
fn kebab_case_converts_spaces() {
    assert_eq!(to_kebab_case("my file name"), "my-file-name");
}

#[test]
fn kebab_case_converts_underscores() {
    assert_eq!(to_kebab_case("my_file_name"), "my-file-name");
}

#[test]
fn kebab_case_collapses_consecutive_hyphens() {
    assert_eq!(to_kebab_case("a _ b"), "a-b");
}

#[test]
fn kebab_case_preserves_lowercase() {
    assert_eq!(to_kebab_case("already-kebab"), "already-kebab");
}

#[test]
fn kebab_case_handles_single_word() {
    assert_eq!(to_kebab_case("Agent"), "agent");
}

#[test]
fn kebab_case_handles_digits() {
    assert_eq!(to_kebab_case("Item2Value"), "item2-value");
}

// --- remap_tools ---

#[test]
fn remap_replaces_backtick_tool_names() {
    let mut mappings = HashMap::new();
    mappings.insert("Read".to_string(), "read_file".to_string());

    let input = "Use `Read` to access files.";
    let result = remap_tools(input, &mappings);
    assert_eq!(result, "Use `read_file` to access files.");
}

#[test]
fn remap_ignores_prose_tool_names() {
    let mut mappings = HashMap::new();
    mappings.insert("Read".to_string(), "read_file".to_string());

    let input = "Read the documentation carefully.";
    let result = remap_tools(input, &mappings);
    assert_eq!(result, "Read the documentation carefully.");
}

#[test]
fn remap_handles_multiple_spans() {
    let mut mappings = HashMap::new();
    mappings.insert("Read".to_string(), "read_file".to_string());
    mappings.insert("Write".to_string(), "write_file".to_string());

    let input = "Use `Read` and `Write` tools.";
    let result = remap_tools(input, &mappings);
    assert_eq!(result, "Use `read_file` and `write_file` tools.");
}

#[test]
fn remap_preserves_unmapped_tools() {
    let mut mappings = HashMap::new();
    mappings.insert("Read".to_string(), "read_file".to_string());

    let input = "Use `Read` and `Agent` tools.";
    let result = remap_tools(input, &mappings);
    assert_eq!(result, "Use `read_file` and `Agent` tools.");
}

#[test]
fn remap_handles_empty_mappings() {
    let mappings = HashMap::new();

    let input = "Use `Read` to access files.";
    let result = remap_tools(input, &mappings);
    assert_eq!(result, "Use `Read` to access files.");
}

#[test]
fn remap_handles_compound_spans() {
    let mut mappings = HashMap::new();
    mappings.insert("Read".to_string(), "read_file".to_string());
    mappings.insert("Write".to_string(), "write_file".to_string());

    let input = "Use `Read/Write` for I/O.";
    let result = remap_tools(input, &mappings);
    assert_eq!(result, "Use `read_file/write_file` for I/O.");
}

#[test]
fn remap_handles_unclosed_backtick() {
    let mut mappings = HashMap::new();
    mappings.insert("Read".to_string(), "read_file".to_string());

    let input = "Broken `Read format";
    let result = remap_tools(input, &mappings);
    assert_eq!(result, "Broken `Read format");
}

// --- markdown_to_toml ---

#[test]
fn to_toml_extracts_description() {
    let content = "---\ndescription: Test agent\n---\n\nBody content.";
    let result = markdown_to_toml("test.md", content).unwrap();
    assert!(result.contains("description = \"Test agent\""));
}

#[test]
fn to_toml_includes_body_as_instructions() {
    let content = "---\ndescription: Test agent\n---\n\nBody content.";
    let result = markdown_to_toml("test.md", content).unwrap();
    assert!(result.contains("instructions = \"\"\""));
    assert!(result.contains("Body content."));
}

#[test]
fn to_toml_includes_source_comment() {
    let content = "---\ndescription: Test agent\n---\n\nBody.";
    let result = markdown_to_toml("Helper.md", content).unwrap();
    assert!(result.starts_with("# source: Helper.md"));
}

#[test]
fn to_toml_handles_missing_description() {
    let content = "---\nname: NoDesc\n---\n\nBody.";
    let result = markdown_to_toml("test.md", content).unwrap();
    assert!(result.contains("description = \"\""));
}

#[test]
fn to_toml_escapes_quotes_in_description() {
    let content = "---\ndescription: A \"quoted\" agent\n---\n\nBody.";
    let result = markdown_to_toml("test.md", content).unwrap();
    assert!(result.contains(r#"description = "A \"quoted\" agent""#));
}

#[test]
fn to_toml_with_agent_fixture() {
    let result = markdown_to_toml("TestAgent.md", AGENT_FIXTURE).unwrap();
    assert!(result.contains("# source: TestAgent.md"));
    assert!(result.contains("Test fixture agent"));
    assert!(result.contains("instructions = \"\"\""));
}

// --- apply_rules ---

#[test]
fn apply_rules_kebab_case_transforms_filename_for_agents() {
    let rules = vec![AssemblyRule::KebabCase];
    let mappings = HashMap::new();

    let (content, filename) =
        apply_rules("body", "SecurityArchitect.md", &rules, &mappings, "agents").unwrap();

    assert_eq!(filename, "security-architect.md");
    assert_eq!(content, "body");
}

#[test]
fn apply_rules_kebab_case_transforms_filename_for_skills() {
    let rules = vec![AssemblyRule::KebabCase];
    let mappings = HashMap::new();

    let (_content, filename) =
        apply_rules("body", "SecurityArchitect.md", &rules, &mappings, "skills").unwrap();

    assert_eq!(filename, "security-architect.md");
}

#[test]
fn apply_rules_kebab_case_transforms_filename_for_rules() {
    let rules = vec![AssemblyRule::KebabCase];
    let mappings = HashMap::new();

    let (_content, filename) =
        apply_rules("body", "SecurityArchitect.md", &rules, &mappings, "rules").unwrap();

    assert_eq!(filename, "security-architect.md");
}

#[test]
fn apply_rules_kebab_case_agents_transforms_filename_for_agents() {
    let rules = vec![AssemblyRule::KebabCaseAgents];
    let mappings = HashMap::new();

    let (_content, filename) =
        apply_rules("body", "SecurityArchitect.md", &rules, &mappings, "agents").unwrap();

    assert_eq!(filename, "security-architect.md");
}

#[test]
fn apply_rules_kebab_case_agents_skips_filename_for_skills() {
    let rules = vec![AssemblyRule::KebabCaseAgents];
    let mappings = HashMap::new();

    let (_content, filename) =
        apply_rules("body", "SecurityArchitect.md", &rules, &mappings, "skills").unwrap();

    assert_eq!(filename, "SecurityArchitect.md");
}

#[test]
fn apply_rules_kebab_case_agents_skips_filename_for_rules() {
    let rules = vec![AssemblyRule::KebabCaseAgents];
    let mappings = HashMap::new();

    let (_content, filename) =
        apply_rules("body", "SecurityArchitect.md", &rules, &mappings, "rules").unwrap();

    assert_eq!(filename, "SecurityArchitect.md");
}

#[test]
fn kebab_case_converts_tax_advisor() {
    assert_eq!(to_kebab_case("TaxAdvisor"), "tax-advisor");
}

#[test]
fn apply_rules_kebab_case_transforms_name_field_for_agents() {
    let rules = vec![AssemblyRule::KebabCase];
    let mappings = HashMap::new();
    let content = "---\nname: SecurityArchitect\n---";

    let (result_content, _filename) =
        apply_rules(content, "SecurityArchitect.md", &rules, &mappings, "agents").unwrap();

    assert!(result_content.contains("name: security-architect"));
}

#[test]
fn apply_rules_remap_transforms_content() {
    let rules = vec![AssemblyRule::RemapTools];
    let mut mappings = HashMap::new();
    mappings.insert("Read".to_string(), "read_file".to_string());

    let (content, filename) =
        apply_rules("Use `Read` tool.", "file.md", &rules, &mappings, "rules").unwrap();

    assert_eq!(content, "Use `read_file` tool.");
    assert_eq!(filename, "file.md");
}

#[test]
fn apply_rules_agents_to_toml_transforms_both() {
    let rules = vec![AssemblyRule::AgentsToToml];
    let mappings = HashMap::new();
    let content = "---\ndescription: Helper\n---\n\nInstructions here.";

    let (result_content, result_filename) =
        apply_rules(content, "Helper.md", &rules, &mappings, "agents").unwrap();

    assert!(result_content.contains("description = \"Helper\""));
    assert_eq!(result_filename, "Helper.toml");
}

#[test]
fn apply_rules_executes_in_order() {
    let rules = vec![AssemblyRule::KebabCase, AssemblyRule::RemapTools];
    let mut mappings = HashMap::new();
    mappings.insert("Read".to_string(), "read_file".to_string());

    let (content, filename) =
        apply_rules("Use `Read`.", "MyAgent.md", &rules, &mappings, "agents").unwrap();

    assert_eq!(filename, "my-agent.md");
    assert_eq!(content, "Use `read_file`.");
}

#[test]
fn apply_rules_empty_rules_returns_unchanged() {
    let rules: Vec<AssemblyRule> = vec![];
    let mappings = HashMap::new();

    let (content, filename) = apply_rules("body", "file.md", &rules, &mappings, "rules").unwrap();

    assert_eq!(content, "body");
    assert_eq!(filename, "file.md");
}

// --- load_tool_mappings (via provider) ---

#[test]
fn load_tool_mappings_parses_gemini() {
    let mappings = crate::provider::load_tool_mappings(REMAP_YAML, "gemini").unwrap();
    assert_eq!(mappings.get("Read").unwrap(), "read_file");
    assert_eq!(mappings.get("Write").unwrap(), "write_file");
    assert_eq!(mappings.get("Bash").unwrap(), "run_shell_command");
}

#[test]
fn load_tool_mappings_returns_empty_for_unknown_provider() {
    let mappings = crate::provider::load_tool_mappings(REMAP_YAML, "nonexistent").unwrap();
    assert!(mappings.is_empty());
}

#[test]
fn load_tool_mappings_returns_empty_for_claude() {
    let mappings = crate::provider::load_tool_mappings(REMAP_YAML, "claude").unwrap();
    assert!(mappings.is_empty());
}
