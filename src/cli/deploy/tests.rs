use super::*;
use tempfile::TempDir;

#[test]
fn validate_target_boundary_accepts_child_path() {
    let temp_directory = TempDir::new().unwrap();
    let child = temp_directory.path().join("child");
    let result = validate_target_boundary(&child, temp_directory.path());
    assert!(result.is_ok());
}

#[test]
fn validate_target_boundary_rejects_escape() {
    let temp_directory = TempDir::new().unwrap();
    let child = temp_directory.path().join("child");
    std::fs::create_dir_all(&child).unwrap();
    let escaped = child.join("../../etc");
    let result = validate_target_boundary(&escaped, &child);
    assert!(result.is_err());
}

#[test]
fn collect_files_recursive_finds_nested_files() {
    let temp_directory = TempDir::new().unwrap();
    let nested = temp_directory.path().join("a/b");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::write(nested.join("file.md"), "content").unwrap();
    std::fs::write(temp_directory.path().join("root.md"), "content").unwrap();

    let files = collect_files_recursive(temp_directory.path()).unwrap();
    assert_eq!(files.len(), 2);
}

#[test]
fn collect_files_recursive_empty_directory() {
    let temp_directory = TempDir::new().unwrap();
    let files = collect_files_recursive(temp_directory.path()).unwrap();
    assert!(files.is_empty());
}

#[test]
fn collect_files_recursive_errors_on_missing_directory() {
    let result = collect_files_recursive(Path::new("/nonexistent/path"));
    assert!(result.is_err());
}

#[test]
fn load_deployed_manifest_returns_empty_for_missing_file() {
    let temp_directory = TempDir::new().unwrap();
    let manifest = load_deployed_manifest(temp_directory.path());
    assert!(manifest.is_empty());
}

#[test]
fn write_manifest_creates_file() {
    let temp_directory = TempDir::new().unwrap();
    let mut entries = HashMap::new();
    entries.insert(
        "rules/UseRTK.md".to_string(),
        manifest::ManifestEntry {
            fingerprint: "abc123".to_string(),
            provenance: None,
        },
    );

    write_manifest(temp_directory.path(), &entries).unwrap();
    assert!(temp_directory.path().join(".manifest").exists());
}

#[test]
fn write_then_load_manifest_roundtrips() {
    let temp_directory = TempDir::new().unwrap();
    let mut entries = HashMap::new();
    entries.insert(
        "rules/UseRTK.md".to_string(),
        manifest::ManifestEntry {
            fingerprint: "abc123".to_string(),
            provenance: Some(".provenance/rules/UseRTK.md.yaml".to_string()),
        },
    );

    write_manifest(temp_directory.path(), &entries).unwrap();
    let loaded = load_deployed_manifest(temp_directory.path());
    assert_eq!(loaded["rules/UseRTK.md"].fingerprint, "abc123");
}
