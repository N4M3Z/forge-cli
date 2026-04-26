use super::*;
use tempfile::TempDir;

const MODULE_YAML: &str = concat!(
    "name: test-module\n",
    "version: 0.1.0\n",
    "description: test\n",
    "events: []\n",
    "repository: https://github.com/test/repo\n",
);

fn write_module_yaml(module_root: &std::path::Path) {
    std::fs::write(module_root.join("module.yaml"), MODULE_YAML).unwrap();
}

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
        true,
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
        true,
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
        true,
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
        true,
    )
    .unwrap();

    assert!(result.installed.is_empty());
}

#[test]
fn execute_writes_provenance_sidecar_by_default() {
    let source = TempDir::new().unwrap();
    let target = TempDir::new().unwrap();

    write_module_yaml(source.path());
    let rules_directory = source.path().join("rules");
    std::fs::create_dir_all(&rules_directory).unwrap();
    std::fs::write(rules_directory.join("KeepChangelog.md"), "# Keep changelog").unwrap();

    execute(
        &source.path().to_string_lossy(),
        &target.path().to_string_lossy(),
        false,
    )
    .unwrap();

    let sidecar = target.path().join("rules/.provenance/KeepChangelog.yaml");
    assert!(
        sidecar.exists(),
        "expected provenance sidecar at {}",
        sidecar.display()
    );

    let statement = std::fs::read_to_string(&sidecar).unwrap();
    let parsed: serde_yaml::Value = serde_yaml::from_str(&statement).unwrap();
    let provenance = &parsed["provenance"];

    assert_eq!(
        provenance["predicate"]["buildDefinition"]["buildType"]
            .as_str()
            .unwrap(),
        &format!("{}/copy/v1", env!("CARGO_PKG_REPOSITORY"))
    );
    assert_eq!(
        provenance["predicate"]["buildDefinition"]["externalParameters"]["source"]
            .as_str()
            .unwrap(),
        "https://github.com/test/repo"
    );
    assert_eq!(
        provenance["subject"][0]["name"].as_str().unwrap(),
        "rules/KeepChangelog.md"
    );
}

#[test]
fn execute_skips_provenance_when_flag_set() {
    let source = TempDir::new().unwrap();
    let target = TempDir::new().unwrap();

    write_module_yaml(source.path());
    let rules_directory = source.path().join("rules");
    std::fs::create_dir_all(&rules_directory).unwrap();
    std::fs::write(rules_directory.join("Foo.md"), "# Foo").unwrap();

    execute(
        &source.path().to_string_lossy(),
        &target.path().to_string_lossy(),
        true,
    )
    .unwrap();

    assert!(target.path().join("rules/Foo.md").exists());
    assert!(!target.path().join("rules/.provenance").exists());
}

#[test]
fn execute_skips_provenance_when_no_module_yaml() {
    let source = TempDir::new().unwrap();
    let target = TempDir::new().unwrap();

    let rules_directory = source.path().join("rules");
    std::fs::create_dir_all(&rules_directory).unwrap();
    std::fs::write(rules_directory.join("Foo.md"), "# Foo").unwrap();

    execute(
        &source.path().to_string_lossy(),
        &target.path().to_string_lossy(),
        false,
    )
    .unwrap();

    assert!(target.path().join("rules/Foo.md").exists());
    assert!(!target.path().join("rules/.provenance").exists());
}

#[test]
fn execute_provenance_digest_matches_content() {
    let source = TempDir::new().unwrap();
    let target = TempDir::new().unwrap();

    write_module_yaml(source.path());
    let rules_directory = source.path().join("rules");
    std::fs::create_dir_all(&rules_directory).unwrap();
    let content = "# Rule body\n";
    std::fs::write(rules_directory.join("Rule.md"), content).unwrap();

    execute(
        &source.path().to_string_lossy(),
        &target.path().to_string_lossy(),
        false,
    )
    .unwrap();

    let statement =
        std::fs::read_to_string(target.path().join("rules/.provenance/Rule.yaml")).unwrap();
    let parsed: serde_yaml::Value = serde_yaml::from_str(&statement).unwrap();

    let expected_digest = commands::manifest::content_sha256(content);
    assert_eq!(
        parsed["provenance"]["subject"][0]["digest"]["sha256"]
            .as_str()
            .unwrap(),
        expected_digest
    );
}
