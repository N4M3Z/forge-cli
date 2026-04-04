use super::*;
use tempfile::TempDir;

#[test]
fn execute_copies_markdown_files() {
    let source = TempDir::new().unwrap();
    let target = TempDir::new().unwrap();

    let rules_directory = source.path().join("rules");
    std::fs::create_dir_all(&rules_directory).unwrap();
    std::fs::write(rules_directory.join("Test.md"), "# Test rule").unwrap();

    let result = execute(
        &source.path().to_string_lossy(),
        &target.path().to_string_lossy(),
    )
    .unwrap();

    assert_eq!(result.installed.len(), 1);
    assert!(target.path().join("rules/Test.md").exists());
}

#[test]
fn execute_skips_non_markdown_files() {
    let source = TempDir::new().unwrap();
    let target = TempDir::new().unwrap();

    let rules_directory = source.path().join("rules");
    std::fs::create_dir_all(&rules_directory).unwrap();
    std::fs::write(rules_directory.join("data.yaml"), "key: value").unwrap();

    let result = execute(
        &source.path().to_string_lossy(),
        &target.path().to_string_lossy(),
    )
    .unwrap();

    assert!(result.installed.is_empty());
    assert!(!target.path().join("rules/data.yaml").exists());
}

#[test]
fn execute_copies_nested_directories() {
    let source = TempDir::new().unwrap();
    let target = TempDir::new().unwrap();

    let nested = source.path().join("rules/cz");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::write(nested.join("Tax.md"), "# Tax rule").unwrap();

    let result = execute(
        &source.path().to_string_lossy(),
        &target.path().to_string_lossy(),
    )
    .unwrap();

    assert_eq!(result.installed.len(), 1);
    assert!(target.path().join("rules/cz/Tax.md").exists());
}

#[test]
fn execute_empty_module_succeeds() {
    let source = TempDir::new().unwrap();
    let target = TempDir::new().unwrap();

    let result = execute(
        &source.path().to_string_lossy(),
        &target.path().to_string_lossy(),
    )
    .unwrap();

    assert!(result.installed.is_empty());
}
