use super::*;
use tempfile::TempDir;

fn write_module_yaml(directory: &std::path::Path) {
    std::fs::write(
        directory.join("module.yaml"),
        "name: test\nversion: 0.1.0\ndescription: test\nevents: []\n",
    )
    .unwrap();
}

#[test]
fn execute_errors_on_missing_module() {
    let result = execute(
        "/nonexistent/module",
        None,
        &[],
        false,
        false,
        false,
        false,
        None,
        None,
        false,
    );
    assert!(result.is_err());
}

#[test]
fn execute_errors_on_directory_without_module_yaml() {
    let directory_without_module = TempDir::new().unwrap();
    let result = execute(
        &directory_without_module.path().to_string_lossy(),
        None,
        &[],
        false,
        false,
        false,
        false,
        None,
        None,
        false,
    );
    let error = result.expect_err("expected install to refuse non-module directory");
    let message = error.to_string();
    assert!(
        message.contains("module.yaml"),
        "error message should mention the missing module.yaml: {message}"
    );
    assert!(
        message.contains("--source"),
        "error message should mention the --source argument: {message}"
    );
}

#[test]
fn execute_succeeds_on_empty_module() {
    let temp_directory = TempDir::new().unwrap();
    std::fs::write(
        temp_directory.path().join("defaults.yaml"),
        "providers:\n    claude:\n        target: .claude\n",
    )
    .unwrap();
    write_module_yaml(temp_directory.path());

    let target = TempDir::new().unwrap();
    let result = execute(
        &temp_directory.path().to_string_lossy(),
        Some(&target.path().to_string_lossy()),
        &[],
        false,
        false,
        false,
        false,
        None,
        None,
        false,
    );
    assert!(result.is_ok());
}

#[test]
fn execute_unknown_provider_lists_available_choices() {
    let module_directory = TempDir::new().unwrap();
    write_module_yaml(module_directory.path());

    let target = TempDir::new().unwrap();
    let result = execute(
        &module_directory.path().to_string_lossy(),
        Some(&target.path().to_string_lossy()),
        &["definitely-not-a-provider".to_string()],
        false,
        false,
        false,
        false,
        None,
        None,
        false,
    );
    let error = result.expect_err("unknown provider must error");
    let message = error.to_string();
    assert!(
        message.contains("definitely-not-a-provider"),
        "error should echo the bad provider name: {message}"
    );
    assert!(
        message.contains("Available:"),
        "error should list available providers: {message}"
    );
    assert!(
        message.contains("claude"),
        "available list should include claude: {message}"
    );
}

#[test]
fn execute_provider_filter_skips_unrequested_providers() {
    let module_directory = TempDir::new().unwrap();
    write_module_yaml(module_directory.path());
    let rules_directory = module_directory.path().join("rules");
    std::fs::create_dir_all(&rules_directory).unwrap();
    std::fs::write(rules_directory.join("OnlyRule.md"), "body\n").unwrap();

    let target = TempDir::new().unwrap();
    let result = execute(
        &module_directory.path().to_string_lossy(),
        Some(&target.path().to_string_lossy()),
        &["opencode".to_string()],
        false,
        false,
        false,
        false,
        None,
        None,
        false,
    )
    .expect("install should succeed for known provider");

    let opencode_rule = target.path().join(".opencode/rules/OnlyRule.md");
    assert!(
        opencode_rule.exists(),
        "opencode rule should be deployed at {}",
        opencode_rule.display()
    );

    for skipped in [".claude", ".gemini", ".codex"] {
        let path = target.path().join(skipped).join("rules/OnlyRule.md");
        assert!(
            !path.exists(),
            "{skipped} should be skipped when --provider opencode given"
        );
    }
    assert!(
        result
            .installed
            .iter()
            .all(|file| file.provider == "opencode"),
        "every deployed file should belong to opencode"
    );
}
