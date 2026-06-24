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

// --- plugin scaffolding (CLI orchestration) ---

fn write_plugin_manifest(root: &std::path::Path, body: &str) {
    let dir = root.join(".claude-plugin");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("plugin.json"), body).unwrap();
}

fn write_hook_script(root: &std::path::Path, relative: &str, executable: bool) {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "#!/bin/sh\necho hi\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = if executable { 0o755 } else { 0o644 };
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode)).unwrap();
    }
    #[cfg(not(unix))]
    let _ = executable;
}

#[test]
fn plugin_scaffolding_skipped_without_manifest() {
    let temp = TempDir::new().unwrap();
    let mut result = ActionResult::new();
    plugin::check_plugin_scaffolding(temp.path(), &mut result);
    assert!(result.errors.is_empty(), "no plugin, no errors");
}

#[test]
fn plugin_scaffolding_valid_tree_passes() {
    let temp = TempDir::new().unwrap();
    write_plugin_manifest(temp.path(), r#"{"name": "my-plugin"}"#);
    std::fs::create_dir_all(temp.path().join("hooks")).unwrap();
    std::fs::write(
        temp.path().join("hooks/hooks.json"),
        r#"{"hooks": {"PreToolUse": [{"hooks": [{"type": "command", "command": "${CLAUDE_PLUGIN_ROOT}/hooks/guard.sh"}]}]}}"#,
    )
    .unwrap();
    write_hook_script(temp.path(), "hooks/guard.sh", true);

    let mut result = ActionResult::new();
    plugin::check_plugin_scaffolding(temp.path(), &mut result);
    assert!(result.errors.is_empty(), "valid tree: {:?}", result.errors);
}

#[test]
fn plugin_scaffolding_corrupt_manifest_errors() {
    let temp = TempDir::new().unwrap();
    write_plugin_manifest(temp.path(), "{ not valid json");

    let mut result = ActionResult::new();
    plugin::check_plugin_scaffolding(temp.path(), &mut result);
    assert!(
        result.errors.iter().any(|e| e.contains("invalid JSON")),
        "expected JSON error: {:?}",
        result.errors
    );
}

#[test]
fn plugin_scaffolding_missing_hook_script_errors() {
    let temp = TempDir::new().unwrap();
    write_plugin_manifest(temp.path(), r#"{"name": "p"}"#);
    std::fs::create_dir_all(temp.path().join("hooks")).unwrap();
    std::fs::write(
        temp.path().join("hooks/hooks.json"),
        r#"{"hooks": {"PreToolUse": [{"hooks": [{"type": "command", "command": "${CLAUDE_PLUGIN_ROOT}/hooks/ghost.sh"}]}]}}"#,
    )
    .unwrap();

    let mut result = ActionResult::new();
    plugin::check_plugin_scaffolding(temp.path(), &mut result);
    assert!(
        result.errors.iter().any(|e| e.contains("not found")),
        "expected missing-script error: {:?}",
        result.errors
    );
}

#[cfg(unix)]
#[test]
fn plugin_scaffolding_non_executable_hook_errors() {
    let temp = TempDir::new().unwrap();
    write_plugin_manifest(temp.path(), r#"{"name": "p"}"#);
    std::fs::create_dir_all(temp.path().join("hooks")).unwrap();
    std::fs::write(
        temp.path().join("hooks/hooks.json"),
        r#"{"hooks": {"PreToolUse": [{"hooks": [{"type": "command", "command": "${CLAUDE_PLUGIN_ROOT}/hooks/guard.sh"}]}]}}"#,
    )
    .unwrap();
    write_hook_script(temp.path(), "hooks/guard.sh", false);

    let mut result = ActionResult::new();
    plugin::check_plugin_scaffolding(temp.path(), &mut result);
    assert!(
        result.errors.iter().any(|e| e.contains("not executable")),
        "expected non-executable error: {:?}",
        result.errors
    );
}
