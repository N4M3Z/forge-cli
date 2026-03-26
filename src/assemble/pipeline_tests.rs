use std::collections::HashMap;

use super::pipeline::{self, SourceFile};
use crate::provider::ProviderConfig;

fn make_source<'a>(relative_path: &'a str, content: &'a str, passthrough: bool) -> SourceFile<'a> {
    SourceFile {
        relative_path,
        content,
        variant_content: None,
        passthrough,
    }
}

fn make_provider(assembly: Option<Vec<&str>>) -> ProviderConfig {
    ProviderConfig {
        target: ".test".to_string(),
        assembly: assembly.map(|v| v.into_iter().map(String::from).collect()),
        deploy: None,
        keep_fields: None,
    }
}

// --- assemble_file ---

#[test]
fn single_file_produces_correct_output_path() {
    let source = make_source("rules/MyRule.md", "body text\n", false);
    let mappings = HashMap::new();

    let result = pipeline::assemble_file(&source, "claude", &[], &["name"], &mappings).unwrap();

    assert_eq!(result.output_path, "claude/rules/MyRule.md");
    assert_eq!(result.source_path, "rules/MyRule.md");
}

#[test]
fn passthrough_skips_content_transform() {
    let content = "---\nname: Test\n---\n\n# Test\n\nBody.\n";
    let source = make_source("rules/Test.md", content, true);
    let mappings = HashMap::new();

    let result = pipeline::assemble_file(&source, "claude", &[], &[], &mappings).unwrap();

    // Passthrough keeps frontmatter and heading intact
    assert!(result.content.contains("---"));
    assert!(result.content.contains("name: Test"));
}

#[test]
fn non_passthrough_strips_frontmatter() {
    let content = "---\nname: Test\nversion: 1\n---\n\nBody text.\n";
    let source = make_source("rules/Test.md", content, false);
    let mappings = HashMap::new();

    let result = pipeline::assemble_file(&source, "claude", &[], &[], &mappings).unwrap();

    assert!(!result.content.contains("---"));
    assert!(!result.content.contains("name:"));
    assert!(result.content.contains("Body text."));
}

#[test]
fn source_hashes_include_source() {
    let source = make_source("rules/R.md", "content", false);
    let mappings = HashMap::new();

    let result = pipeline::assemble_file(&source, "p", &[], &[], &mappings).unwrap();

    assert_eq!(result.source_hashes.len(), 1);
    assert_eq!(result.source_hashes[0].0, "rules/R.md");
}

#[test]
fn source_hashes_include_variant() {
    let source = SourceFile {
        relative_path: "rules/R.md",
        content: "base",
        variant_content: Some("---\nmode: append\n---\nextra"),
        passthrough: false,
    };
    let mappings = HashMap::new();

    let result = pipeline::assemble_file(&source, "p", &[], &[], &mappings).unwrap();

    assert_eq!(result.source_hashes.len(), 2);
    assert_eq!(result.source_hashes[1].0, "rules/R.md (variant)");
}

#[test]
fn kebab_case_rule_transforms_output_filename() {
    let source = make_source("agents/SecurityArchitect.md", "body\n", false);
    let rules = vec![crate::provider::AssemblyRule::KebabCase];
    let mappings = HashMap::new();

    let result = pipeline::assemble_file(&source, "gemini", &rules, &[], &mappings).unwrap();

    assert_eq!(result.output_path, "gemini/agents/security-architect.md");
}

// --- assemble_module ---

#[test]
fn multi_provider_produces_separate_outputs() {
    let sources = vec![make_source("rules/Rule.md", "body\n", false)];

    let mut providers = HashMap::new();
    providers.insert("claude".to_string(), make_provider(None));
    providers.insert(
        "gemini".to_string(),
        make_provider(Some(vec!["kebab-case"])),
    );

    let tool_mappings: HashMap<String, HashMap<String, String>> = HashMap::new();

    let (results, errors) = pipeline::assemble_module(&sources, &providers, &tool_mappings, &[]);

    assert!(errors.is_empty());
    assert_eq!(results.len(), 2);

    let paths: Vec<&str> = results.iter().map(|r| r.output_path.as_str()).collect();
    assert!(paths.contains(&"claude/rules/Rule.md"));
    assert!(paths.contains(&"gemini/rules/rule.md"));
}

#[test]
fn unknown_rule_collected_as_error() {
    let sources = vec![make_source("rules/R.md", "body\n", false)];

    let mut providers = HashMap::new();
    providers.insert(
        "bad".to_string(),
        ProviderConfig {
            target: ".bad".to_string(),
            assembly: Some(vec!["nonexistent-rule".to_string()]),
            deploy: None,
            keep_fields: None,
        },
    );

    let tool_mappings: HashMap<String, HashMap<String, String>> = HashMap::new();

    let (results, errors) = pipeline::assemble_module(&sources, &providers, &tool_mappings, &[]);

    assert!(!errors.is_empty());
    assert!(errors[0].contains("unknown assembly rule"));
    // Files still assembled (unknown rule skipped, valid rules applied)
    assert_eq!(results.len(), 1);
}

#[test]
fn empty_sources_produces_empty_results() {
    let sources: Vec<SourceFile> = vec![];
    let mut providers = HashMap::new();
    providers.insert("claude".to_string(), make_provider(None));

    let tool_mappings: HashMap<String, HashMap<String, String>> = HashMap::new();

    let (results, errors) = pipeline::assemble_module(&sources, &providers, &tool_mappings, &[]);

    assert!(results.is_empty());
    assert!(errors.is_empty());
}
