use assert_cmd::Command;
use std::fs;
use std::path::Path;

fn forge() -> Command {
    Command::cargo_bin("forge").unwrap()
}

/// Create a minimal module in a temp directory.
fn scaffold_module(root: &Path) {
    fs::write(
        root.join("module.yaml"),
        "name: test-module\nversion: 0.1.0\ndescription: test module\nevents: []\n",
    )
    .unwrap();

    fs::write(
        root.join("defaults.yaml"),
        "skills:\n    claude:\n        TestSkill:\n",
    )
    .unwrap();
}

fn create_agent(root: &Path, name: &str, model: &str) {
    let agents_dir = root.join("agents");
    fs::create_dir_all(&agents_dir).unwrap();
    fs::write(
        agents_dir.join(format!("{name}.md")),
        format!(
            "---\nname: {name}\ndescription: test agent for deployment verification\nmodel: {model}\n---\n\nAgent instructions here.\n"
        ),
    )
    .unwrap();
}

fn create_rule(root: &Path, name: &str) {
    let rules_dir = root.join("rules");
    fs::create_dir_all(&rules_dir).unwrap();
    fs::write(
        rules_dir.join(format!("{name}.md")),
        format!(
            "---\nname: {name}\ndescription: test rule\n---\n\nRule content with a reference [1].\n\n[1]: https://example.com\n"
        ),
    )
    .unwrap();
}

fn create_nested_rule(root: &Path, subdirectory: &str, name: &str) {
    let rules_dir = root.join("rules").join(subdirectory);
    fs::create_dir_all(&rules_dir).unwrap();
    fs::write(
        rules_dir.join(format!("{name}.md")),
        format!("---\nname: {name}\ndescription: nested rule\n---\n\nNested rule content.\n"),
    )
    .unwrap();
}

fn create_skill(root: &Path, name: &str) {
    let skill_dir = root.join("skills").join(name);
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(
        skill_dir.join("SKILL.md"),
        format!(
            "---\nname: {name}\ndescription: test skill\nversion: 1.0.0\n---\n\nSkill instructions.\n"
        ),
    )
    .unwrap();
}

fn create_skill_with_companion(root: &Path, name: &str, companion: &str) {
    create_skill(root, name);
    let skill_dir = root.join("skills").join(name);
    fs::write(
        skill_dir.join(companion),
        "Companion content for the skill.\n",
    )
    .unwrap();
}

// --- Install tests ---

#[test]
fn install_deploys_agent_to_all_providers() {
    let module_directory = tempfile::tempdir().unwrap();
    let target_directory = tempfile::tempdir().unwrap();

    scaffold_module(module_directory.path());
    create_agent(module_directory.path(), "TestAgent", "strong");

    forge()
        .args([
            "install",
            module_directory.path().to_str().unwrap(),
            "--target",
            target_directory.path().to_str().unwrap(),
        ])
        .assert()
        .success();

    assert!(
        target_directory
            .path()
            .join(".claude/agents/TestAgent.md")
            .is_file()
    );
    assert!(
        target_directory
            .path()
            .join(".gemini/agents/test-agent.md")
            .is_file()
    );
    assert!(
        target_directory
            .path()
            .join(".codex/agents/TestAgent.md")
            .is_file()
    );
    assert!(
        target_directory
            .path()
            .join(".opencode/agents/test-agent.md")
            .is_file()
    );
}

#[test]
fn install_maps_model_tier_for_claude() {
    let module_directory = tempfile::tempdir().unwrap();
    let target_directory = tempfile::tempdir().unwrap();

    scaffold_module(module_directory.path());
    create_agent(module_directory.path(), "StrongAgent", "strong");

    forge()
        .args([
            "install",
            module_directory.path().to_str().unwrap(),
            "--target",
            target_directory.path().to_str().unwrap(),
        ])
        .assert()
        .success();

    let deployed = fs::read_to_string(
        target_directory
            .path()
            .join(".claude/agents/StrongAgent.md"),
    )
    .unwrap();

    assert!(deployed.contains("model: opus"));
    assert!(!deployed.contains("model: strong"));
}

#[test]
fn install_strips_rule_frontmatter_for_claude() {
    let module_directory = tempfile::tempdir().unwrap();
    let target_directory = tempfile::tempdir().unwrap();

    scaffold_module(module_directory.path());
    create_rule(module_directory.path(), "TestRule");

    forge()
        .args([
            "install",
            module_directory.path().to_str().unwrap(),
            "--target",
            target_directory.path().to_str().unwrap(),
        ])
        .assert()
        .success();

    let deployed =
        fs::read_to_string(target_directory.path().join(".claude/rules/TestRule.md")).unwrap();

    assert!(!deployed.contains("---"));
    assert!(deployed.contains("Rule content"));
}

#[test]
fn install_keeps_links_for_claude_strips_for_gemini() {
    let module_directory = tempfile::tempdir().unwrap();
    let target_directory = tempfile::tempdir().unwrap();

    scaffold_module(module_directory.path());
    create_rule(module_directory.path(), "LinkedRule");

    forge()
        .args([
            "install",
            module_directory.path().to_str().unwrap(),
            "--target",
            target_directory.path().to_str().unwrap(),
        ])
        .assert()
        .success();

    let claude_rule =
        fs::read_to_string(target_directory.path().join(".claude/rules/LinkedRule.md")).unwrap();
    let gemini_rule =
        fs::read_to_string(target_directory.path().join(".gemini/rules/linked-rule.md")).unwrap();

    assert!(claude_rule.contains("[1]: https://example.com"));
    assert!(!gemini_rule.contains("[1]:"));
}

#[test]
fn install_deploys_nested_rules() {
    let module_directory = tempfile::tempdir().unwrap();
    let target_directory = tempfile::tempdir().unwrap();

    scaffold_module(module_directory.path());
    create_nested_rule(module_directory.path(), "sub", "NestedRule");

    forge()
        .args([
            "install",
            module_directory.path().to_str().unwrap(),
            "--target",
            target_directory.path().to_str().unwrap(),
        ])
        .assert()
        .success();

    assert!(
        target_directory
            .path()
            .join(".claude/rules/sub/NestedRule.md")
            .is_file()
    );
}

#[test]
fn install_deploys_skill_with_companion() {
    let module_directory = tempfile::tempdir().unwrap();
    let target_directory = tempfile::tempdir().unwrap();

    scaffold_module(module_directory.path());
    create_skill_with_companion(module_directory.path(), "TestSkill", "Reference.md");

    forge()
        .args([
            "install",
            module_directory.path().to_str().unwrap(),
            "--target",
            target_directory.path().to_str().unwrap(),
        ])
        .assert()
        .success();

    assert!(
        target_directory
            .path()
            .join(".claude/skills/TestSkill/SKILL.md")
            .is_file()
    );
    assert!(
        target_directory
            .path()
            .join(".claude/skills/TestSkill/Reference.md")
            .is_file()
    );
}

// --- Manifest tests ---

#[test]
fn install_creates_nested_manifest() {
    let module_directory = tempfile::tempdir().unwrap();
    let target_directory = tempfile::tempdir().unwrap();

    scaffold_module(module_directory.path());
    create_agent(module_directory.path(), "Agent", "fast");
    create_rule(module_directory.path(), "Rule");

    forge()
        .args([
            "install",
            module_directory.path().to_str().unwrap(),
            "--target",
            target_directory.path().to_str().unwrap(),
        ])
        .assert()
        .success();

    let manifest = fs::read_to_string(target_directory.path().join(".claude/.manifest")).unwrap();

    assert!(manifest.contains("agents:"));
    assert!(manifest.contains("  Agent.md:"));
    assert!(manifest.contains("    fingerprint:"));
    assert!(manifest.contains("    provenance:"));
    assert!(manifest.contains("rules:"));
}

// --- Provenance tests ---

#[test]
fn install_deploys_provenance_sidecars() {
    let module_directory = tempfile::tempdir().unwrap();
    let target_directory = tempfile::tempdir().unwrap();

    scaffold_module(module_directory.path());
    create_agent(module_directory.path(), "TracedAgent", "fast");

    forge()
        .args([
            "install",
            module_directory.path().to_str().unwrap(),
            "--target",
            target_directory.path().to_str().unwrap(),
        ])
        .assert()
        .success();

    let provenance_path = target_directory
        .path()
        .join(".claude/agents/.provenance/TracedAgent.yaml");

    assert!(provenance_path.is_file());

    let provenance = fs::read_to_string(&provenance_path).unwrap();
    assert!(provenance.contains("in-toto.io/Statement/v1"));
    assert!(provenance.contains("externalParameters:"));
    assert!(provenance.contains("source: test-module"));
}

#[test]
fn install_deploys_nested_provenance() {
    let module_directory = tempfile::tempdir().unwrap();
    let target_directory = tempfile::tempdir().unwrap();

    scaffold_module(module_directory.path());
    create_nested_rule(module_directory.path(), "deep", "DeepRule");

    forge()
        .args([
            "install",
            module_directory.path().to_str().unwrap(),
            "--target",
            target_directory.path().to_str().unwrap(),
        ])
        .assert()
        .success();

    assert!(
        target_directory
            .path()
            .join(".claude/rules/deep/.provenance/DeepRule.yaml")
            .is_file()
    );
}

// --- Idempotency tests ---

#[test]
fn install_is_idempotent() {
    let module_directory = tempfile::tempdir().unwrap();
    let target_directory = tempfile::tempdir().unwrap();

    scaffold_module(module_directory.path());
    create_agent(module_directory.path(), "IdempotentAgent", "fast");

    // First install
    forge()
        .args([
            "install",
            module_directory.path().to_str().unwrap(),
            "--target",
            target_directory.path().to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains("deployed"));

    // Second install — should skip
    forge()
        .args([
            "install",
            module_directory.path().to_str().unwrap(),
            "--target",
            target_directory.path().to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicates::str::contains("skipped"));
}

// --- Empty module ---

#[test]
fn install_empty_module_succeeds() {
    let module_directory = tempfile::tempdir().unwrap();
    let target_directory = tempfile::tempdir().unwrap();

    scaffold_module(module_directory.path());

    forge()
        .args([
            "install",
            module_directory.path().to_str().unwrap(),
            "--target",
            target_directory.path().to_str().unwrap(),
        ])
        .assert()
        .success();
}

// --- Validate tests ---

#[test]
fn validate_reports_missing_required_files() {
    let module_directory = tempfile::tempdir().unwrap();

    // Only module.yaml — missing defaults.yaml, README.md, LICENSE
    fs::write(
        module_directory.path().join("module.yaml"),
        "name: incomplete\nversion: 0.1.0\n",
    )
    .unwrap();
    fs::write(
        module_directory.path().join("defaults.yaml"),
        "skills:\n    claude:\n        Skill:\n",
    )
    .unwrap();

    forge()
        .args(["validate", module_directory.path().to_str().unwrap()])
        .assert()
        .failure()
        .stdout(predicates::str::contains("MISSING README.md"))
        .stdout(predicates::str::contains("MISSING LICENSE"));
}

#[test]
fn validate_passes_complete_module() {
    let module_directory = tempfile::tempdir().unwrap();

    scaffold_module(module_directory.path());
    fs::write(module_directory.path().join("README.md"), "# Test\n").unwrap();
    fs::write(module_directory.path().join("LICENSE"), "EUPL-1.2\n").unwrap();

    forge()
        .args(["validate", module_directory.path().to_str().unwrap()])
        .assert()
        .success();
}

// --- Copy tests ---

#[test]
fn copy_preserves_frontmatter() {
    let module_directory = tempfile::tempdir().unwrap();
    let target_directory = tempfile::tempdir().unwrap();

    scaffold_module(module_directory.path());
    create_agent(module_directory.path(), "RawAgent", "strong");

    forge()
        .args([
            "copy",
            module_directory.path().to_str().unwrap(),
            "--target",
            target_directory.path().to_str().unwrap(),
        ])
        .assert()
        .success();

    let copied = fs::read_to_string(target_directory.path().join("agents/RawAgent.md")).unwrap();

    assert!(copied.contains("---"));
    assert!(copied.contains("model: strong"));
}
