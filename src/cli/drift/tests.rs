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
fn compare_file_content_h1_heading_stripped_is_not_drift() {
    // Upstream has H1 heading, deployed (module) has it stripped by assembly.
    // Drift should NOT report body drift for this.
    let module_content = "---\nname: test\n---\nBody after heading.";
    let upstream_content = "---\nname: test\n---\n# TestSkill\nBody after heading.";
    let entry = compare_file_content(
        "SKILL.md",
        module_content,
        upstream_content,
        "skills",
        &HashSet::new(),
    );
    assert_eq!(entry.status, DriftStatus::Identical);
}

#[test]
fn compare_file_content_h1_rename_is_invisible_to_drift() {
    // Heading renames are intentionally invisible: headings are assembly
    // artifacts derived from the `name` frontmatter field, not authored content.
    let module_content = "---\nname: test\n---\n# OldName\nSame body.";
    let upstream_content = "---\nname: test\n---\n# NewName\nSame body.";
    let entry = compare_file_content(
        "SKILL.md",
        module_content,
        upstream_content,
        "skills",
        &HashSet::new(),
    );
    assert_eq!(entry.status, DriftStatus::Identical);
}

#[test]
fn compare_file_content_h1_stripped_but_body_modified_is_drift() {
    // Heading normalization must NOT mask real body changes.
    let module_content = "---\nname: test\n---\nModified body.";
    let upstream_content = "---\nname: test\n---\n# TestSkill\nOriginal body.";
    let entry = compare_file_content(
        "SKILL.md",
        module_content,
        upstream_content,
        "skills",
        &HashSet::new(),
    );
    assert_eq!(entry.status, DriftStatus::BodyOnly);
}

#[test]
fn collect_markdown_files_returns_empty_for_missing_directory() {
    let files = collect_markdown_files(Path::new("/nonexistent"));
    assert!(files.is_empty());
}
