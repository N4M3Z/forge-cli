use super::*;
use tempfile::TempDir;

#[test]
fn check_module_structure_reports_missing_required_files() {
    let temp_directory = TempDir::new().unwrap();
    let mut result = ActionResult::new();

    check_module_structure(temp_directory.path(), &mut result);

    assert_eq!(result.errors.len(), REQUIRED_FILES.len());
}

#[test]
fn check_module_structure_passes_with_all_required_files() {
    let temp_directory = TempDir::new().unwrap();

    for filename in REQUIRED_FILES {
        std::fs::write(temp_directory.path().join(filename), "content").unwrap();
    }

    let mut result = ActionResult::new();
    check_module_structure(temp_directory.path(), &mut result);

    assert!(result.errors.is_empty());
}

#[test]
fn check_module_yaml_validates_against_embedded_schema() {
    let temp_directory = TempDir::new().unwrap();
    let module_yaml = "name: test-module\nversion: 0.1.0\ndescription: test\nevents: []\n";
    std::fs::write(temp_directory.path().join("module.yaml"), module_yaml).unwrap();

    let mut result = ActionResult::new();
    check_module_yaml(temp_directory.path(), &mut result);

    assert!(
        result.errors.is_empty(),
        "unexpected errors: {:?}",
        result.errors
    );
}

#[test]
fn check_module_yaml_skips_when_no_module_yaml() {
    let temp_directory = TempDir::new().unwrap();
    let mut result = ActionResult::new();

    check_module_yaml(temp_directory.path(), &mut result);

    assert!(result.errors.is_empty());
}

#[test]
fn skill_directory_accepts_claude_code_optional_fields() {
    let temp_directory = TempDir::new().unwrap();
    let skill_dir = temp_directory.path().join("ExampleSkill");
    std::fs::create_dir_all(&skill_dir).unwrap();

    let skill_md = "---\n\
name: ExampleSkill\n\
description: \"Test skill using Claude Code optional frontmatter fields.\"\n\
version: 0.1.0\n\
argument-hint: \"[year]\"\n\
allowed-tools: [Read, Bash]\n\
model: claude-opus-4-7\n\
effort: high\n\
when_to_use: \"When the user asks for X.\"\n\
---\n\
\n\
# ExampleSkill\n\
\n\
Body content for the test skill.\n";
    std::fs::write(skill_dir.join("SKILL.md"), skill_md).unwrap();

    let mut result = ActionResult::new();
    check::skill_directory(&skill_dir, &mut result).unwrap();

    assert!(
        result.errors.is_empty(),
        "Claude Code optional skill fields should validate cleanly: {:?}",
        result.errors
    );
}

// --- tools.rs native checks ---

#[test]
fn is_excluded_matches_prefix_glob() {
    let module_root = std::path::Path::new("/project");
    let file_path = std::path::Path::new("/project/templates/statement.yaml");
    let patterns = vec!["templates/*".to_string()];
    assert!(tools::is_excluded(file_path, module_root, &patterns));
}

#[test]
fn is_excluded_rejects_non_matching_path() {
    let module_root = std::path::Path::new("/project");
    let file_path = std::path::Path::new("/project/schemas/agent.schema.yaml");
    let patterns = vec!["templates/*".to_string()];
    assert!(!tools::is_excluded(file_path, module_root, &patterns));
}

#[test]
fn is_excluded_matches_exact_path() {
    let module_root = std::path::Path::new("/project");
    let file_path = std::path::Path::new("/project/defaults.yaml");
    let patterns = vec!["defaults.yaml".to_string()];
    assert!(tools::is_excluded(file_path, module_root, &patterns));
}
