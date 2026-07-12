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
    let manifest = load_deployed_manifest(temp_directory.path()).unwrap();
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
    let loaded = load_deployed_manifest(temp_directory.path()).unwrap();
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

#[test]
fn deploy_provider_files_only_prefix_filters_deployment() {
    let temp_directory = TempDir::new().unwrap();
    let build_dir = temp_directory.path().join("build/claude");
    std::fs::create_dir_all(build_dir.join("skills/Alpha")).unwrap();
    std::fs::create_dir_all(build_dir.join("skills/Beta")).unwrap();
    std::fs::write(build_dir.join("skills/Alpha/SKILL.md"), "alpha body").unwrap();
    std::fs::write(build_dir.join("skills/Beta/SKILL.md"), "beta body").unwrap();
    let target = temp_directory.path().join("target");

    let mut manifest_entries = HashMap::new();
    let mut deployed_keys = HashSet::new();
    let mut result = ActionResult::new();
    deploy_provider_kind_files(
        &build_dir.join("skills"),
        commands::provider::ContentKind::Skills,
        &target,
        &mut manifest_entries,
        &mut deployed_keys,
        &mut result,
        "claude",
        false,
        Some("skills/Alpha/"),
    )
    .unwrap();

    assert!(target.join("skills/Alpha/SKILL.md").is_file());
    assert!(!target.join("skills/Beta/SKILL.md").exists());
    assert!(deployed_keys.contains("skills/Alpha/SKILL.md"));
    assert!(!deployed_keys.contains("skills/Beta/SKILL.md"));
    assert_eq!(
        std::fs::read_to_string(target.join("skills/Alpha/SKILL.md")).unwrap(),
        "alpha body"
    );
}

#[test]
fn only_matches_respects_boundaries() {
    assert!(only_matches("skills/Alpha/SKILL.md", "skills/Alpha/"));
    assert!(only_matches("skills/Alpha/SKILL.md", "skills/Alpha"));
    assert!(!only_matches("skills/AlphaOther/SKILL.md", "skills/Alpha"));
    assert!(only_matches("agents/Name.md", "agents/Name."));
    assert!(only_matches("agents/Name.toml", "agents/Name"));
    assert!(!only_matches("agents/NameOther.md", "agents/Name"));
}

#[test]
fn only_matches_survives_provider_slugging() {
    assert!(only_matches(
        "agents/security-architect.md",
        "agents/SecurityArchitect"
    ));
    assert!(!only_matches(
        "agents/security-architect-two.md",
        "agents/SecurityArchitect"
    ));
}

#[test]
fn ensure_destination_within_rejects_symlink_escape() {
    let temp_directory = TempDir::new().unwrap();
    let base = temp_directory.path().join("base");
    let outside = temp_directory.path().join("outside");
    std::fs::create_dir_all(base.join("skills")).unwrap();
    std::fs::create_dir_all(&outside).unwrap();
    std::os::unix::fs::symlink(&outside, base.join("skills/Escape")).unwrap();

    assert!(ensure_destination_within(&base.join("skills/Inside/SKILL.md"), &base).is_ok());
    assert!(ensure_destination_within(&base.join("skills/Escape/SKILL.md"), &base).is_err());
}
