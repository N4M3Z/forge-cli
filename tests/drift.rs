use assert_cmd::Command;
use std::fs;
use std::path::Path;

fn forge() -> Command {
    Command::cargo_bin("forge").unwrap()
}

fn write_file(root: &Path, relative_path: &str, content: &str) {
    let full_path = root.join(relative_path);
    fs::create_dir_all(full_path.parent().unwrap()).unwrap();
    fs::write(full_path, content).unwrap();
}

fn rule_with_frontmatter(name: &str, body: &str) -> String {
    format!("---\nname: {name}\ndescription: test rule for drift verification\n---\n\n{body}\n")
}

fn agent_with_model(name: &str, model: &str, body: &str) -> String {
    format!(
        "---\nname: {name}\ndescription: test agent for drift verification\nmodel: {model}\n---\n\n{body}\n"
    )
}

// --- Identical files ---

#[test]
fn drift_identical_rules_reports_zero_exit() {
    let module_directory = tempfile::tempdir().unwrap();
    let upstream_directory = tempfile::tempdir().unwrap();

    let content = rule_with_frontmatter(
        "KeepChangelog",
        "When making a notable change, add an entry to CHANGELOG.md.",
    );

    write_file(module_directory.path(), "rules/KeepChangelog.md", &content);
    write_file(
        upstream_directory.path(),
        "rules/KeepChangelog.md",
        &content,
    );

    forge()
        .args([
            "drift",
            module_directory.path().to_str().unwrap(),
            upstream_directory.path().to_str().unwrap(),
        ])
        .assert()
        .success();
}

#[test]
fn drift_identical_files_shown_in_json() {
    let module_directory = tempfile::tempdir().unwrap();
    let upstream_directory = tempfile::tempdir().unwrap();

    let content = rule_with_frontmatter("TestRule", "Identical body content.");

    write_file(module_directory.path(), "rules/TestRule.md", &content);
    write_file(upstream_directory.path(), "rules/TestRule.md", &content);

    let output = forge()
        .args([
            "drift",
            module_directory.path().to_str().unwrap(),
            upstream_directory.path().to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("\"Identical\""),
        "expected Identical status in JSON: {stdout}"
    );
}

// --- Body drift ---

#[test]
fn drift_body_difference_detected() {
    let module_directory = tempfile::tempdir().unwrap();
    let upstream_directory = tempfile::tempdir().unwrap();

    write_file(
        module_directory.path(),
        "rules/UseRTK.md",
        &rule_with_frontmatter("UseRTK", "Always prefix shell commands with rtk."),
    );
    write_file(
        upstream_directory.path(),
        "rules/UseRTK.md",
        &rule_with_frontmatter(
            "UseRTK",
            "Always prefix shell commands with rtk. RTK does not support -C.",
        ),
    );

    let output = forge()
        .args([
            "drift",
            module_directory.path().to_str().unwrap(),
            upstream_directory.path().to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("\"BodyOnly\""),
        "expected BodyOnly status: {stdout}"
    );
    assert!(
        stdout.contains("\"changed_keys\": []"),
        "body-only drift should have empty changed_keys: {stdout}"
    );
    assert!(
        !output.status.success(),
        "drift should return nonzero exit code"
    );
}

// --- Frontmatter drift ---

#[test]
fn drift_frontmatter_difference_detected() {
    let module_directory = tempfile::tempdir().unwrap();
    let upstream_directory = tempfile::tempdir().unwrap();

    let body = "Architecture Decision Records capture the why behind structural choices.";

    write_file(
        module_directory.path(),
        "rules/ArchitectureDecisionRecords.md",
        &format!(
            "---\nname: ArchitectureDecisionRecords\ndescription: local variant\n---\n\n{body}\n"
        ),
    );
    write_file(
        upstream_directory.path(),
        "rules/ArchitectureDecisionRecords.md",
        &format!(
            "---\nname: ArchitectureDecisionRecords\ndescription: upstream variant with extra field\nversion: 1.0\n---\n\n{body}\n"
        ),
    );

    let output = forge()
        .args([
            "drift",
            module_directory.path().to_str().unwrap(),
            upstream_directory.path().to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("\"FrontmatterOnly\""),
        "expected FrontmatterOnly status: {stdout}"
    );
    assert!(
        stdout.contains("\"description\""),
        "expected description in changed_keys: {stdout}"
    );
    assert!(
        stdout.contains("\"version\""),
        "expected version in changed_keys: {stdout}"
    );
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let changed_keys = parsed["entries"][0]["changed_keys"].as_array().unwrap();
    let key_strings: Vec<&str> = changed_keys
        .iter()
        .filter_map(serde_json::Value::as_str)
        .collect();
    assert!(
        !key_strings.contains(&"name"),
        "name is identical, should not be in changed_keys: {key_strings:?}"
    );
}

// --- Both differ ---

#[test]
fn drift_both_frontmatter_and_body_difference() {
    let module_directory = tempfile::tempdir().unwrap();
    let upstream_directory = tempfile::tempdir().unwrap();

    write_file(
        module_directory.path(),
        "rules/Diverged.md",
        &rule_with_frontmatter("Diverged", "Local body content."),
    );
    write_file(
        upstream_directory.path(),
        "rules/Diverged.md",
        &format!(
            "---\nname: Diverged\ndescription: different description\nversion: 2.0\n---\n\nUpstream body content.\n"
        ),
    );

    let output = forge()
        .args([
            "drift",
            module_directory.path().to_str().unwrap(),
            upstream_directory.path().to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("\"Both\""),
        "expected Both status: {stdout}"
    );
}

// --- Local only ---

#[test]
fn drift_local_only_file_detected() {
    let module_directory = tempfile::tempdir().unwrap();
    let upstream_directory = tempfile::tempdir().unwrap();

    write_file(
        module_directory.path(),
        "rules/LocalRule.md",
        &rule_with_frontmatter("LocalRule", "Only exists locally."),
    );

    // upstream has no rules/ at all
    fs::create_dir_all(upstream_directory.path().join("rules")).unwrap();

    let output = forge()
        .args([
            "drift",
            module_directory.path().to_str().unwrap(),
            upstream_directory.path().to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("\"LocalOnly\""),
        "expected LocalOnly status: {stdout}"
    );

    // local-only files should NOT cause nonzero exit
    assert!(
        output.status.success(),
        "local-only files should not cause drift failure"
    );
}

// --- Upstream only ---

#[test]
fn drift_upstream_only_file_detected() {
    let module_directory = tempfile::tempdir().unwrap();
    let upstream_directory = tempfile::tempdir().unwrap();

    fs::create_dir_all(module_directory.path().join("rules")).unwrap();

    write_file(
        upstream_directory.path(),
        "rules/UpstreamRule.md",
        &rule_with_frontmatter("UpstreamRule", "Only exists upstream."),
    );

    let output = forge()
        .args([
            "drift",
            module_directory.path().to_str().unwrap(),
            upstream_directory.path().to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("\"UpstreamOnly\""),
        "expected UpstreamOnly status: {stdout}"
    );
    assert!(
        !output.status.success(),
        "upstream-only files indicate drift"
    );
}

// --- Skills ---

#[test]
fn drift_skill_body_difference() {
    let module_directory = tempfile::tempdir().unwrap();
    let upstream_directory = tempfile::tempdir().unwrap();

    write_file(
        module_directory.path(),
        "skills/ArchitectureDecision/SKILL.md",
        "---\nname: ArchitectureDecision\ndescription: test skill\n---\n\nLocal skill instructions.\n",
    );
    write_file(
        upstream_directory.path(),
        "skills/ArchitectureDecision/SKILL.md",
        "---\nname: ArchitectureDecision\ndescription: test skill\n---\n\nUpstream skill instructions with additions.\n",
    );

    let output = forge()
        .args([
            "drift",
            module_directory.path().to_str().unwrap(),
            upstream_directory.path().to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("\"BodyOnly\""),
        "expected BodyOnly for skill: {stdout}"
    );
    assert!(
        stdout.contains("ArchitectureDecision/SKILL.md"),
        "should show relative path: {stdout}"
    );
}

// --- Agents ---

#[test]
fn drift_agent_model_difference_is_frontmatter() {
    let module_directory = tempfile::tempdir().unwrap();
    let upstream_directory = tempfile::tempdir().unwrap();

    let body = "Review code for quality issues.";

    write_file(
        module_directory.path(),
        "agents/CodeReviewer.md",
        &agent_with_model("CodeReviewer", "strong", body),
    );
    write_file(
        upstream_directory.path(),
        "agents/CodeReviewer.md",
        &agent_with_model("CodeReviewer", "fast", body),
    );

    let output = forge()
        .args([
            "drift",
            module_directory.path().to_str().unwrap(),
            upstream_directory.path().to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("\"FrontmatterOnly\""),
        "expected FrontmatterOnly for agent model diff: {stdout}"
    );
}

// --- Decisions ---

#[test]
fn drift_decisions_compared() {
    let module_directory = tempfile::tempdir().unwrap();
    let upstream_directory = tempfile::tempdir().unwrap();

    let adr_content = "---\ntitle: Test Decision\nstatus: accepted\ndate: 2026-01-01\n---\n\n# Test Decision\n\nDecision body.\n";

    write_file(
        module_directory.path(),
        "docs/decisions/ADR-0001 Test.md",
        adr_content,
    );
    write_file(
        upstream_directory.path(),
        "docs/decisions/ADR-0001 Test.md",
        adr_content,
    );

    let output = forge()
        .args([
            "drift",
            module_directory.path().to_str().unwrap(),
            upstream_directory.path().to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("\"decisions\""),
        "expected decisions category: {stdout}"
    );
    assert!(
        stdout.contains("\"Identical\""),
        "expected Identical status: {stdout}"
    );
}

// --- No frontmatter ---

#[test]
fn drift_files_without_frontmatter_compared_as_full_body() {
    let module_directory = tempfile::tempdir().unwrap();
    let upstream_directory = tempfile::tempdir().unwrap();

    write_file(
        module_directory.path(),
        "rules/NoFrontmatter.md",
        "Plain content without fences.\n",
    );
    write_file(
        upstream_directory.path(),
        "rules/NoFrontmatter.md",
        "Different plain content.\n",
    );

    let output = forge()
        .args([
            "drift",
            module_directory.path().to_str().unwrap(),
            upstream_directory.path().to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("\"BodyOnly\""),
        "no-frontmatter diff should be BodyOnly: {stdout}"
    );
}

// --- Nested rules ---

#[test]
fn drift_nested_rules_compared() {
    let module_directory = tempfile::tempdir().unwrap();
    let upstream_directory = tempfile::tempdir().unwrap();

    let content = rule_with_frontmatter("PersonalTaxIncome", "Czech personal income tax rules.");

    write_file(
        module_directory.path(),
        "rules/cz/PersonalTaxIncome.md",
        &content,
    );
    write_file(
        upstream_directory.path(),
        "rules/cz/PersonalTaxIncome.md",
        &content,
    );

    let output = forge()
        .args([
            "drift",
            module_directory.path().to_str().unwrap(),
            upstream_directory.path().to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("cz/PersonalTaxIncome.md"),
        "should include subdirectory in name: {stdout}"
    );
    assert!(
        stdout.contains("\"Identical\""),
        "nested identical files: {stdout}"
    );
}

// --- Empty directories ---

#[test]
fn drift_empty_module_against_populated_upstream() {
    let module_directory = tempfile::tempdir().unwrap();
    let upstream_directory = tempfile::tempdir().unwrap();

    write_file(
        upstream_directory.path(),
        "rules/SomeRule.md",
        &rule_with_frontmatter("SomeRule", "Upstream rule."),
    );

    let output = forge()
        .args([
            "drift",
            module_directory.path().to_str().unwrap(),
            upstream_directory.path().to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("\"UpstreamOnly\""),
        "expected UpstreamOnly: {stdout}"
    );
}

// --- Mixed content kinds ---

#[test]
fn drift_reports_all_content_kinds() {
    let module_directory = tempfile::tempdir().unwrap();
    let upstream_directory = tempfile::tempdir().unwrap();

    let rule_content = rule_with_frontmatter("SharedRule", "Shared rule body.");
    let agent_content = agent_with_model("SharedAgent", "strong", "Shared agent body.");
    let skill_content =
        "---\nname: SharedSkill\ndescription: test skill\n---\n\nShared skill body.\n";

    write_file(
        module_directory.path(),
        "rules/SharedRule.md",
        &rule_content,
    );
    write_file(
        upstream_directory.path(),
        "rules/SharedRule.md",
        &rule_content,
    );

    write_file(
        module_directory.path(),
        "agents/SharedAgent.md",
        &agent_content,
    );
    write_file(
        upstream_directory.path(),
        "agents/SharedAgent.md",
        &agent_content,
    );

    write_file(
        module_directory.path(),
        "skills/SharedSkill/SKILL.md",
        skill_content,
    );
    write_file(
        upstream_directory.path(),
        "skills/SharedSkill/SKILL.md",
        skill_content,
    );

    let output = forge()
        .args([
            "drift",
            module_directory.path().to_str().unwrap(),
            upstream_directory.path().to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("\"agents\""),
        "should have agents category: {stdout}"
    );
    assert!(
        stdout.contains("\"skills\""),
        "should have skills category: {stdout}"
    );
    assert!(
        stdout.contains("\"rules\""),
        "should have rules category: {stdout}"
    );
}

// --- Ignore keys ---

#[test]
fn ignore_frontmatter_keys_marks_expected() {
    let module_directory = tempfile::tempdir().unwrap();
    let upstream_directory = tempfile::tempdir().unwrap();

    let body = "Decision body content.";

    write_file(
        module_directory.path(),
        "docs/decisions/ADR-0001 Test.md",
        &format!("---\ntitle: Test\nproject: proton-agents\nauthor: Alice\n---\n\n{body}\n"),
    );
    write_file(
        upstream_directory.path(),
        "docs/decisions/ADR-0001 Test.md",
        &format!("---\ntitle: Test\nproject: forge-core\nauthor: Bob\n---\n\n{body}\n"),
    );

    let output = forge()
        .args([
            "drift",
            module_directory.path().to_str().unwrap(),
            upstream_directory.path().to_str().unwrap(),
            "--ignore",
            "project,author",
            "--json",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("\"Expected\""),
        "all ignored keys should produce Expected: {stdout}"
    );
    assert!(
        output.status.success(),
        "Expected drift should not cause nonzero exit"
    );
}

#[test]
fn ignore_body_marks_expected() {
    let module_directory = tempfile::tempdir().unwrap();
    let upstream_directory = tempfile::tempdir().unwrap();

    write_file(
        module_directory.path(),
        "rules/TestRule.md",
        &rule_with_frontmatter("TestRule", "Local body."),
    );
    write_file(
        upstream_directory.path(),
        "rules/TestRule.md",
        &rule_with_frontmatter("TestRule", "Upstream body."),
    );

    let output = forge()
        .args([
            "drift",
            module_directory.path().to_str().unwrap(),
            upstream_directory.path().to_str().unwrap(),
            "--ignore",
            "body",
            "--json",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("\"Expected\""),
        "ignored body should produce Expected: {stdout}"
    );
}

#[test]
fn ignore_partial_keys_keeps_drift() {
    let module_directory = tempfile::tempdir().unwrap();
    let upstream_directory = tempfile::tempdir().unwrap();

    let body = "Same body.";

    write_file(
        module_directory.path(),
        "rules/TestRule.md",
        &format!("---\nname: TestRule\nproject: local\nversion: 2.0\n---\n\n{body}\n"),
    );
    write_file(
        upstream_directory.path(),
        "rules/TestRule.md",
        &format!("---\nname: TestRule\nproject: upstream\nversion: 1.0\n---\n\n{body}\n"),
    );

    let output = forge()
        .args([
            "drift",
            module_directory.path().to_str().unwrap(),
            upstream_directory.path().to_str().unwrap(),
            "--ignore",
            "project",
            "--json",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("\"FrontmatterOnly\""),
        "non-ignored key 'version' should keep drift: {stdout}"
    );
}

#[test]
fn ignore_both_frontmatter_and_body() {
    let module_directory = tempfile::tempdir().unwrap();
    let upstream_directory = tempfile::tempdir().unwrap();

    write_file(
        module_directory.path(),
        "rules/TestRule.md",
        &format!("---\nname: TestRule\nproject: local\n---\n\nLocal body.\n"),
    );
    write_file(
        upstream_directory.path(),
        "rules/TestRule.md",
        &format!("---\nname: TestRule\nproject: upstream\n---\n\nUpstream body.\n"),
    );

    let output = forge()
        .args([
            "drift",
            module_directory.path().to_str().unwrap(),
            upstream_directory.path().to_str().unwrap(),
            "--ignore",
            "project,body",
            "--json",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("\"Expected\""),
        "all drift ignored should produce Expected: {stdout}"
    );
}

#[test]
fn ignore_body_on_both_drift_keeps_frontmatter() {
    let module_directory = tempfile::tempdir().unwrap();
    let upstream_directory = tempfile::tempdir().unwrap();

    write_file(
        module_directory.path(),
        "rules/TestRule.md",
        &format!("---\nname: TestRule\nversion: 2.0\n---\n\nLocal body.\n"),
    );
    write_file(
        upstream_directory.path(),
        "rules/TestRule.md",
        &format!("---\nname: TestRule\nversion: 1.0\n---\n\nUpstream body.\n"),
    );

    let output = forge()
        .args([
            "drift",
            module_directory.path().to_str().unwrap(),
            upstream_directory.path().to_str().unwrap(),
            "--ignore",
            "body",
            "--json",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("\"FrontmatterOnly\""),
        "ignoring body on Both should downgrade to FrontmatterOnly: {stdout}"
    );
}
