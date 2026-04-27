use super::*;

#[test]
fn split_parts_separates_frontmatter_and_body() {
    let content = "---\nname: test\n---\n\nBody text here.";
    let (frontmatter, body) = split_parts(content);
    assert!(frontmatter.contains("name: test"));
    assert!(body.contains("Body text here."));
}

#[test]
fn split_parts_returns_full_content_as_body_without_frontmatter() {
    let content = "No frontmatter here.";
    let (frontmatter, body) = split_parts(content);
    assert!(frontmatter.is_empty());
    assert_eq!(body, content);
}

#[test]
fn diff_frontmatter_keys_detects_changed_value() {
    let module_yaml = "name: forge-cli\nversion: 0.2.0\n";
    let upstream_yaml = "name: forge-cli\nversion: 0.1.0\n";
    let changed = diff_frontmatter_keys(module_yaml, upstream_yaml);
    assert_eq!(changed, vec!["version"]);
}

#[test]
fn diff_frontmatter_keys_detects_added_key() {
    let module_yaml = "name: test\nauthor: alice\n";
    let upstream_yaml = "name: test\n";
    let changed = diff_frontmatter_keys(module_yaml, upstream_yaml);
    assert!(changed.contains(&"author".to_string()));
}

#[test]
fn diff_frontmatter_keys_returns_empty_when_identical() {
    let yaml = "name: test\nversion: 0.1.0\n";
    let changed = diff_frontmatter_keys(yaml, yaml);
    assert!(changed.is_empty());
}

#[test]
fn apply_ignore_filter_marks_frontmatter_as_expected() {
    let status = apply_ignore_filter(
        DriftStatus::FrontmatterOnly,
        &["project".to_string()],
        &["project"].into_iter().collect(),
    );
    assert_eq!(status, DriftStatus::Expected);
}

#[test]
fn apply_ignore_filter_marks_body_as_expected() {
    let status = apply_ignore_filter(DriftStatus::BodyOnly, &[], &["body"].into_iter().collect());
    assert_eq!(status, DriftStatus::Expected);
}

#[test]
fn apply_ignore_filter_both_reduces_to_body_when_keys_ignored() {
    let status = apply_ignore_filter(
        DriftStatus::Both,
        &["author".to_string()],
        &["author"].into_iter().collect(),
    );
    assert_eq!(status, DriftStatus::BodyOnly);
}

#[test]
fn apply_ignore_filter_both_reduces_to_frontmatter_when_body_ignored() {
    let status = apply_ignore_filter(
        DriftStatus::Both,
        &["author".to_string()],
        &["body"].into_iter().collect(),
    );
    assert_eq!(status, DriftStatus::FrontmatterOnly);
}

#[test]
fn apply_ignore_filter_no_ignored_returns_unchanged() {
    let status = apply_ignore_filter(
        DriftStatus::FrontmatterOnly,
        &["project".to_string()],
        &HashSet::new(),
    );
    assert_eq!(status, DriftStatus::FrontmatterOnly);
}

#[test]
fn compare_file_content_identical_files() {
    let content = "---\nname: test\n---\n\nBody.";
    let entry = compare_file_content("test.md", content, content, "rules", &HashSet::new());
    assert_eq!(entry.status, DriftStatus::Identical);
}

#[test]
fn compare_file_content_body_only_drift() {
    let module_content = "---\nname: test\n---\n\nLocal body.";
    let upstream_content = "---\nname: test\n---\n\nUpstream body.";
    let entry = compare_file_content(
        "test.md",
        module_content,
        upstream_content,
        "rules",
        &HashSet::new(),
    );
    assert_eq!(entry.status, DriftStatus::BodyOnly);
}

#[test]
fn compare_file_content_frontmatter_only_drift() {
    let module_content = "---\nname: local\n---\n\nSame body.";
    let upstream_content = "---\nname: upstream\n---\n\nSame body.";
    let entry = compare_file_content(
        "test.md",
        module_content,
        upstream_content,
        "rules",
        &HashSet::new(),
    );
    assert_eq!(entry.status, DriftStatus::FrontmatterOnly);
    assert!(entry.changed_keys.contains(&"name".to_string()));
}

#[test]
fn parse_top_level_keys_extracts_flat_yaml() {
    let yaml = "name: test\nversion: 0.1.0\n";
    let keys = parse_top_level_keys(yaml);
    assert!(keys.contains_key("name"));
    assert!(keys.contains_key("version"));
}

#[test]
fn parse_top_level_keys_returns_empty_for_invalid_yaml() {
    let keys = parse_top_level_keys("not: [valid: yaml");
    assert!(keys.is_empty());
}

#[test]
fn collect_markdown_files_returns_empty_for_missing_directory() {
    let files = collect_markdown_files(Path::new("/nonexistent"));
    assert!(files.is_empty());
}

fn write_sidecar(directory: &Path, stem: &str, subject: &str, source: &str) {
    let provenance_directory = directory.join(".provenance");
    std::fs::create_dir_all(&provenance_directory).unwrap();
    let sidecar = format!(
        "provenance:\n    _type: https://in-toto.io/Statement/v1\n    subject:\n        - name: {subject}\n          digest:\n              sha256: deadbeef\n    predicate:\n        buildDefinition:\n            buildType: https://example.test/copy/v1\n            externalParameters:\n                source: {source}\n            resolvedDependencies:\n                - uri: {subject}\n                  digest:\n                      sha256: deadbeef\n        runDetails:\n            builder:\n                id: forge-cli\n                version:\n                    forge: 0.0.0-test\n            metadata:\n                startedOn: \"2026-01-01T00:00:00Z\"\n"
    );
    std::fs::write(provenance_directory.join(format!("{stem}.yaml")), sidecar).unwrap();
}

#[test]
fn drift_surfaces_source_uri_on_same_name_match() {
    let module = tempfile::TempDir::new().unwrap();
    let upstream = tempfile::TempDir::new().unwrap();

    let module_rules = module.path().join("rules");
    let upstream_rules = upstream.path().join("rules");
    std::fs::create_dir_all(&module_rules).unwrap();
    std::fs::create_dir_all(&upstream_rules).unwrap();
    std::fs::write(module_rules.join("AgentTeams.md"), "# Agent teams\n").unwrap();
    std::fs::write(upstream_rules.join("AgentTeams.md"), "# Agent teams\n").unwrap();
    write_sidecar(
        &module_rules,
        "AgentTeams",
        "rules/AgentTeams.md",
        "https://github.com/N4M3Z/forge-core",
    );

    let mut result = DriftResult::default();
    compare_directory_pair(
        &mut result,
        &module_rules,
        &upstream_rules,
        "rules",
        "rules",
        &HashSet::new(),
    );

    assert_eq!(result.entries.len(), 1, "expected one entry");
    let entry = &result.entries[0];
    assert_eq!(entry.name, "AgentTeams.md");
    assert_eq!(entry.status, DriftStatus::Identical);
    assert_eq!(
        entry.source_uri.as_deref(),
        Some("https://github.com/N4M3Z/forge-core")
    );
    assert!(entry.renamed_from.is_none());
}

#[test]
fn drift_resolves_renamed_adoption() {
    let module = tempfile::TempDir::new().unwrap();
    let upstream = tempfile::TempDir::new().unwrap();

    let module_rules = module.path().join("rules");
    let upstream_rules = upstream.path().join("rules");
    std::fs::create_dir_all(&module_rules).unwrap();
    std::fs::create_dir_all(&upstream_rules).unwrap();
    std::fs::write(module_rules.join("SecretsScan.md"), "# Scanner\n").unwrap();
    std::fs::write(upstream_rules.join("SecretScan.md"), "# Scanner\n").unwrap();
    write_sidecar(
        &module_rules,
        "SecretsScan",
        "SecretScan.md",
        "https://github.com/N4M3Z/forge-core",
    );

    let mut result = DriftResult::default();
    compare_directory_pair(
        &mut result,
        &module_rules,
        &upstream_rules,
        "rules",
        "rules",
        &HashSet::new(),
    );

    assert_eq!(
        result.entries.len(),
        1,
        "rename should collapse two names into one entry, got {:?}",
        result.entries
    );
    let entry = &result.entries[0];
    assert_eq!(entry.name, "SecretsScan.md");
    assert_eq!(entry.status, DriftStatus::Identical);
    assert_eq!(entry.renamed_from.as_deref(), Some("SecretScan.md"));
    assert_eq!(
        entry.source_uri.as_deref(),
        Some("https://github.com/N4M3Z/forge-core")
    );
}
