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
    assert!(manifest_content.contains("provenance:"));
}

#[test]
fn init_writes_provenance_sidecars() {
    let temp_directory = TempDir::new().unwrap();
    execute(&temp_directory.path().to_string_lossy()).unwrap();

    let sidecar = temp_directory.path().join(".provenance/LICENSE.yaml");
    assert!(sidecar.is_file());

    let content = std::fs::read_to_string(&sidecar).unwrap();
    assert!(content.contains("https://in-toto.io/Statement/v1"));
    assert!(content.contains("templates/init/LICENSE"));
}

#[test]
fn init_excludes_customized_files_from_manifest() {
    let temp_directory = TempDir::new().unwrap();
    std::fs::write(temp_directory.path().join("README.md"), "# Custom\n").unwrap();

    execute(&temp_directory.path().to_string_lossy()).unwrap();

    let manifest_content =
        std::fs::read_to_string(temp_directory.path().join(".manifest")).unwrap();
    assert!(
        !manifest_content.contains("README.md"),
        "customized pre-existing file should not be in manifest"
    );
}

#[test]
fn init_deploys_hidden_template_files() {
    let temp_directory = TempDir::new().unwrap();
    execute(&temp_directory.path().to_string_lossy()).unwrap();

    for required in [
        ".pre-commit-config.yaml",
        ".gitattributes",
        ".gitleaks.toml",
        ".gitlab-ci.yml",
    ] {
        let path = temp_directory.path().join(required);
        assert!(
            path.is_file(),
            "expected hidden template file to be deployed: {required}"
        );
    }
}

#[test]
fn is_os_junk_blocks_macos_and_windows_artifacts() {
    assert!(is_os_junk(".DS_Store"));
    assert!(is_os_junk("subdir/.DS_Store"));
    assert!(is_os_junk("Thumbs.db"));
    assert!(is_os_junk("Desktop.ini"));
    assert!(is_os_junk("._hidden_macos_resource_fork"));
}

#[test]
fn is_os_junk_does_not_block_legitimate_dotfiles() {
    assert!(!is_os_junk(".pre-commit-config.yaml"));
    assert!(!is_os_junk(".gitattributes"));
    assert!(!is_os_junk(".gitleaks.toml"));
    assert!(!is_os_junk(".gitlab-ci.yml"));
    assert!(!is_os_junk(".githooks/pre-commit"));
    assert!(!is_os_junk(".github/workflows/release.yml"));
    assert!(!is_os_junk("agents/.mdschema"));
}

#[test]
fn init_uses_already_exists_skip_reason() {
    let temp_directory = TempDir::new().unwrap();
    std::fs::write(temp_directory.path().join("LICENSE"), "custom\n").unwrap();

    let result = execute(&temp_directory.path().to_string_lossy()).unwrap();

    let license_skip = result
        .skipped
        .iter()
        .find(|s| s.target.contains("LICENSE"))
        .expect("LICENSE should be skipped");
    assert!(matches!(license_skip.reason, SkipReason::AlreadyExists));
}
