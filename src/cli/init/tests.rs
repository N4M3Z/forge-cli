use super::*;
use tempfile::TempDir;

#[test]
fn init_creates_all_files() {
    let temp_directory = TempDir::new().unwrap();
    let result = execute(&temp_directory.path().to_string_lossy()).unwrap();

    assert!(!result.installed.is_empty());
    assert!(temp_directory.path().join("module.yaml").is_file());
    assert!(temp_directory.path().join("defaults.yaml").is_file());
    assert!(temp_directory.path().join("README.md").is_file());
    assert!(temp_directory.path().join("LICENSE").is_file());
    assert!(temp_directory.path().join("Makefile").is_file());
    assert!(temp_directory.path().join(".githooks/pre-commit").is_file());
    assert!(temp_directory.path().join("agents/.mdschema").is_file());
    assert!(temp_directory.path().join("rules/.mdschema").is_file());
}

#[test]
fn init_substitutes_module_name() {
    let temp_directory = TempDir::new().unwrap();
    execute(&temp_directory.path().to_string_lossy()).unwrap();

    let module_yaml = std::fs::read_to_string(temp_directory.path().join("module.yaml")).unwrap();
    assert!(!module_yaml.contains("${MODULE_NAME}"));
    assert!(module_yaml.contains("name:"));
}

#[test]
fn init_skips_existing_files() {
    let temp_directory = TempDir::new().unwrap();
    std::fs::write(temp_directory.path().join("README.md"), "# Custom\n").unwrap();

    let result = execute(&temp_directory.path().to_string_lossy()).unwrap();

    let readme = std::fs::read_to_string(temp_directory.path().join("README.md")).unwrap();
    assert_eq!(readme, "# Custom\n");
    assert!(
        result
            .skipped
            .iter()
            .any(|skipped| skipped.target.contains("README.md"))
    );
}

#[test]
fn init_writes_manifest() {
    let temp_directory = TempDir::new().unwrap();
    execute(&temp_directory.path().to_string_lossy()).unwrap();

    let manifest_path = temp_directory.path().join(".manifest");
    assert!(manifest_path.is_file());

    let manifest_content = std::fs::read_to_string(&manifest_path).unwrap();
    assert!(manifest_content.contains("fingerprint:"));
}
