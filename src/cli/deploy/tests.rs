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

// --- parse_repo ---

#[test]
fn parse_repo_extracts_https_url() {
    assert_eq!(
        parse_repo("https://github.com/N4M3Z/forge-core"),
        Some((
            "github.com".to_string(),
            "N4M3Z".to_string(),
            "forge-core".to_string()
        ))
    );
}

#[test]
fn parse_repo_strips_dot_git_suffix() {
    assert_eq!(
        parse_repo("https://github.com/N4M3Z/forge-core.git"),
        Some((
            "github.com".to_string(),
            "N4M3Z".to_string(),
            "forge-core".to_string()
        ))
    );
}

#[test]
fn parse_repo_handles_git_at_form() {
    assert_eq!(
        parse_repo("git@github.com:N4M3Z/forge-core.git"),
        Some((
            "github.com".to_string(),
            "N4M3Z".to_string(),
            "forge-core".to_string()
        ))
    );
}

#[test]
fn parse_repo_tolerates_trailing_slash() {
    assert_eq!(
        parse_repo("https://github.com/N4M3Z/forge-core/"),
        Some((
            "github.com".to_string(),
            "N4M3Z".to_string(),
            "forge-core".to_string()
        ))
    );
}

#[test]
fn parse_repo_returns_none_for_bare_name() {
    assert_eq!(parse_repo("forge-core"), None);
    assert_eq!(parse_repo("PublishPrompts"), None);
}

#[test]
fn parse_repo_distinguishes_same_name_different_owner() {
    let a = parse_repo("https://github.com/N4M3Z/forge-core").unwrap();
    let b = parse_repo("https://github.com/other-org/forge-core").unwrap();
    assert_ne!(a, b);
}

#[test]
fn parse_repo_distinguishes_same_name_different_host() {
    let a = parse_repo("https://github.com/N4M3Z/forge-core").unwrap();
    let b = parse_repo("https://gitlab.com/N4M3Z/forge-core").unwrap();
    assert_ne!(a, b);
}

// --- prune_empty_parents ---

#[test]
fn prune_empty_parents_removes_chain() {
    let root = TempDir::new().unwrap();
    let stop = root.path();
    let nested = stop.join("a/b/c");
    std::fs::create_dir_all(&nested).unwrap();
    let file = nested.join("file");
    std::fs::write(&file, "x").unwrap();
    std::fs::remove_file(&file).unwrap();

    prune_empty_parents(Some(&nested), stop);

    assert!(!nested.exists());
    assert!(!stop.join("a/b").exists());
    assert!(!stop.join("a").exists());
    assert!(stop.exists(), "stop directory must survive");
}

#[test]
fn prune_empty_parents_stops_at_non_empty() {
    let root = TempDir::new().unwrap();
    let stop = root.path();
    let nested = stop.join("a/b");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::write(stop.join("a/sibling"), "stay").unwrap();

    prune_empty_parents(Some(&nested), stop);

    assert!(!nested.exists(), "empty leaf removed");
    assert!(stop.join("a").exists(), "non-empty parent preserved");
    assert!(stop.join("a/sibling").exists(), "sibling file untouched");
}

#[test]
fn prune_empty_parents_never_removes_stop() {
    let root = TempDir::new().unwrap();
    let stop = root.path();
    let nested = stop.join("only");
    std::fs::create_dir_all(&nested).unwrap();

    prune_empty_parents(Some(&nested), stop);

    assert!(!nested.exists());
    assert!(stop.exists(), "stop directory must never be removed");
}
