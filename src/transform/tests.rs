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
    let content =
        "---\nname: TestAgent\ndescription: Test agent\nmodel: gpt-5.4\n---\n\nBody content.";
    let result = markdown_to_toml("test.md", content).unwrap();
    assert!(result.contains("description = \"Test agent\""));
}

#[test]
fn to_toml_includes_body_as_instructions() {
    let content =
        "---\nname: TestAgent\ndescription: Test agent\nmodel: gpt-5.4\n---\n\nBody content.";
    let result = markdown_to_toml("test.md", content).unwrap();
    assert!(result.contains("developer_instructions = "));
    assert!(result.contains("Body content."));
}

#[test]
fn to_toml_includes_source_comment() {
    let content = "---\nname: TestAgent\ndescription: Test agent\nmodel: gpt-5.4\n---\n\nBody.";
    let result = markdown_to_toml("Helper.md", content).unwrap();
    assert!(result.starts_with("# source: Helper.md\n"));
}

#[test]
fn to_toml_handles_missing_description() {
    let content = "---\nname: NoDesc\nmodel: gpt-5.4\n---\n\nBody.";
    let result = markdown_to_toml("test.md", content).unwrap();
    assert!(result.contains("description = \"\""));
}

#[test]
fn to_toml_escapes_quotes_in_description() {
    let content =
        "---\nname: TestAgent\ndescription: A \"quoted\" agent\nmodel: gpt-5.4\n---\n\nBody.";
    let result = markdown_to_toml("test.md", content).unwrap();
    let parsed: toml::Value = toml::from_str(&result).unwrap();
    assert_eq!(
        parsed.get("description").and_then(toml::Value::as_str),
        Some("A \"quoted\" agent")
    );
}

#[test]
fn to_toml_with_agent_fixture() {
    let result = markdown_to_toml("TestAgent.md", AGENT_FIXTURE).unwrap();
    assert!(result.starts_with("# source: TestAgent.md\n"));
    assert!(result.contains("Test fixture agent"));
    assert!(result.contains("developer_instructions = "));
}

#[test]
fn to_toml_includes_effort_when_present() {
    let content =
        "---\nname: TestAgent\ndescription: Test agent\nmodel: gpt-5.4\neffort: low\n---\n\nBody.";
    let result = markdown_to_toml("test.md", content).unwrap();
    assert!(result.contains("model_reasoning_effort = \"low\""));
}

#[test]
fn to_toml_serializes_regex_heavy_body() {
    let content = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/input/codex-toml-stress.md"
    ));
    let result = markdown_to_toml("TomlStressFixture.md", content).unwrap();
    let parsed: toml::Value = toml::from_str(&result).unwrap();
    let instructions = parsed
        .get("developer_instructions")
        .and_then(toml::Value::as_str)
        .unwrap();
    assert!(instructions.contains(r#"(alpha|beta|gamma|delta)\s*=\s*['"][^'"]{8,}"#));
    assert!(instructions.contains(r"\path\to\file"));
    assert!(instructions.contains(r"[injected_section]"));
    assert!(instructions.contains(r#""""#));
}

#[test]
fn to_toml_escapes_triple_quote_injection_attempt() {
    let body = "Normal preamble.\n\"\"\"\n[malicious_section]\nname = \"injected\"\npath = \"C:\\Windows\"\n\"\"\"\nstray \" quote\nTrailing line.";
    let content =
        format!("---\nname: TestAgent\ndescription: stress\nmodel: gpt-5.4\n---\n\n{body}");
    let result = markdown_to_toml("test.md", &content).unwrap();

    let parsed: toml::Value = toml::from_str(&result).unwrap();
    let table = parsed.as_table().unwrap();

    assert!(!table.contains_key("malicious_section"));

    let instructions = table
        .get("developer_instructions")
        .and_then(toml::Value::as_str)
        .unwrap();
    assert!(instructions.contains("[malicious_section]"));
    assert!(instructions.contains("\"\"\""));
    assert!(instructions.contains("stray \" quote"));
}

#[test]
fn to_toml_emits_multi_line_basic_string_for_body() {
    let content = "---\nname: TestAgent\ndescription: stress\nmodel: gpt-5.4\n---\n\nLine one.\nLine two.\nLine three.\n";
    let result = markdown_to_toml("test.md", content).unwrap();

    // Multi-line BASIC strings ("""..."""), not multi-line LITERAL strings
    // ('''...'''). Basic strings escape embedded `"""`, which is what closes
    // the injection sink; literal strings have no escapes and would reopen it.
    assert!(
        result.contains("developer_instructions = \"\"\"\n"),
        "expected multi-line basic TOML string, got:\n{result}"
    );
    assert!(
        !result.contains("developer_instructions = '''"),
        "must not be multi-line literal form"
    );
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
fn apply_rules_agents_to_toml_converts_agents() {
    let rules = vec![AssemblyRule::AgentsToToml];
    let mappings = HashMap::new();
    let content = "---\nname: TestAgent\ndescription: Test agent\n---\n\nBody.";

    let (out, filename) =
        apply_rules(content, "TestAgent.md", &rules, &mappings, "agents").unwrap();

    assert_eq!(filename, "TestAgent.toml");
    assert!(out.contains("description = \"Test agent\""));
}

#[test]
fn apply_rules_agents_to_toml_leaves_skills_as_markdown() {
    let rules = vec![AssemblyRule::AgentsToToml];
    let mappings = HashMap::new();
    let content = "---\nname: WebDevelopment\ndescription: Web skill\n---\n\nBody.";

    let (out, filename) = apply_rules(content, "SKILL.md", &rules, &mappings, "skills").unwrap();

    assert_eq!(filename, "SKILL.md");
    assert_eq!(out, content);
}

#[test]
fn apply_rules_agents_to_toml_leaves_rules_as_markdown() {
    let rules = vec![AssemblyRule::AgentsToToml];
    let mappings = HashMap::new();
    let content = "---\nname: NoEmDash\n---\n\nBody.";

    let (out, filename) = apply_rules(content, "NoEmDash.md", &rules, &mappings, "rules").unwrap();

    assert_eq!(filename, "NoEmDash.md");
    assert_eq!(out, content);
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
