//! Integration tests for the prune flow: ownership match, quarantine, dry-run,
//! companion-file sweep, empty-parent walk, and provider-tree parametrization.

use assert_cmd::Command;
use std::fs;
use std::path::{Path, PathBuf};

fn forge() -> Command {
    Command::cargo_bin("forge").unwrap()
}

/// Write `module.yaml` with an explicit `repository:` URL so the deployed
/// provenance sidecar carries it. The substring-collision fix depends on
/// the structured equality of these URIs.
fn scaffold_module_with_repo(root: &Path, name: &str, repo_url: &str) {
    fs::write(
        root.join("module.yaml"),
        format!(
            "name: {name}\nversion: 0.1.0\ndescription: prune test\nevents: []\nrepository: {repo_url}\n"
        ),
    )
    .unwrap();
    fs::write(root.join("defaults.yaml"), "").unwrap();
}

/// Scaffold without a `repository:` field so `source_uri()` falls back to the
/// bare module name. Used to exercise the bare-name vs structured-URL branch.
fn scaffold_module_bare(root: &Path, name: &str) {
    fs::write(
        root.join("module.yaml"),
        format!("name: {name}\nversion: 0.1.0\ndescription: prune test\nevents: []\n"),
    )
    .unwrap();
    fs::write(root.join("defaults.yaml"), "").unwrap();
}

fn create_skill(root: &Path, name: &str) {
    let skill_dir = root.join("skills").join(name);
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(
        skill_dir.join("SKILL.md"),
        format!(
            "---\nname: {name}\ndescription: prune fixture skill\nversion: 0.1.0\n---\n\nBody.\n"
        ),
    )
    .unwrap();
}

fn create_skill_with_companion(root: &Path, name: &str, companion: &str) {
    create_skill(root, name);
    let companion_path = root.join("skills").join(name).join(companion);
    fs::write(&companion_path, "Companion body.\n").unwrap();
}

fn install(source: &Path, target: &Path, extra_args: &[&str]) -> assert_cmd::assert::Assert {
    let mut args = vec![
        "install",
        "--source",
        source.to_str().unwrap(),
        "--target",
        target.to_str().unwrap(),
    ];
    args.extend(extra_args);
    forge().args(args).assert()
}

fn list_trash(target: &Path, provider: &str) -> Vec<PathBuf> {
    let trash_root = target.join(provider).join(".trash");
    if !trash_root.is_dir() {
        return Vec::new();
    }
    fs::read_dir(&trash_root)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect()
}

// --- substring collision regression ---

#[test]
fn prune_respects_provenance_source_uri() {
    let module_a = tempfile::tempdir().unwrap();
    let module_b = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();

    scaffold_module_with_repo(
        module_a.path(),
        "forge-core",
        "https://github.com/N4M3Z/forge-core",
    );
    create_skill(module_a.path(), "AlphaSkill");

    scaffold_module_with_repo(
        module_b.path(),
        "forge-core",
        "https://github.com/other-org/forge-core",
    );
    create_skill(module_b.path(), "BetaSkill");

    // Install module-a; AlphaSkill lands on the target.
    install(module_a.path(), target.path(), &[]).success();
    assert!(
        target
            .path()
            .join(".claude/skills/AlphaSkill/SKILL.md")
            .exists(),
        "module-a's AlphaSkill should deploy"
    );

    // Install module-b with prune. Its manifest lists only BetaSkill.
    // AlphaSkill's provenance carries module-a's repo URL, so it must
    // survive module-b's prune pass.
    install(module_b.path(), target.path(), &[]).success();
    assert!(
        target
            .path()
            .join(".claude/skills/AlphaSkill/SKILL.md")
            .exists(),
        "AlphaSkill from module-a must survive module-b's prune (different repository URL)"
    );
    assert!(
        target
            .path()
            .join(".claude/skills/BetaSkill/SKILL.md")
            .exists(),
        "BetaSkill should deploy from module-b"
    );
}

#[test]
fn prune_does_not_match_name_suffix() {
    // Module A is "PublishPrompts"; module B is "Prompts".
    // Pre-fix, "Prompts" matched "PublishPrompts" via ends_with("/Prompts").
    let module_a = tempfile::tempdir().unwrap();
    let module_b = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();

    scaffold_module_with_repo(
        module_a.path(),
        "PublishPrompts",
        "https://github.com/example/PublishPrompts",
    );
    create_skill(module_a.path(), "Foo");

    scaffold_module_with_repo(
        module_b.path(),
        "Prompts",
        "https://github.com/example/Prompts",
    );
    create_skill(module_b.path(), "Bar");

    install(module_a.path(), target.path(), &[]).success();
    install(module_b.path(), target.path(), &[]).success();

    assert!(
        target.path().join(".claude/skills/Foo/SKILL.md").exists(),
        "PublishPrompts/Foo must survive Prompts's prune (no substring match)"
    );
    assert!(
        target.path().join(".claude/skills/Bar/SKILL.md").exists(),
        "Prompts/Bar should deploy"
    );
}

// --- two-pass prune across all providers ---

/// Skills deploy as `SKILL.md` for every provider, Codex included; only agents
/// are converted to TOML (`agents-to-toml` is gated to `kind == "agents"`).
fn skill_file_extension(_provider: &str) -> &'static str {
    "md"
}

fn run_two_pass_prune_for_provider(provider: &str) {
    let module = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    let ext = skill_file_extension(provider);

    scaffold_module_with_repo(
        module.path(),
        "test-module",
        "https://github.com/example/test-module",
    );
    create_skill(module.path(), "AlphaSkill");
    create_skill_with_companion(module.path(), "BetaSkill", "Helper.md");

    // Pass 1: install both skills.
    install(module.path(), target.path(), &[]).success();
    let deployed_alpha = target
        .path()
        .join(provider)
        .join(format!("skills/AlphaSkill/SKILL.{ext}"));
    let deployed_beta = target
        .path()
        .join(provider)
        .join(format!("skills/BetaSkill/SKILL.{ext}"));
    let deployed_helper = target
        .path()
        .join(provider)
        .join(format!("skills/BetaSkill/Helper.{ext}"));
    assert!(
        deployed_alpha.is_file(),
        "{provider}: AlphaSkill must deploy"
    );
    assert!(deployed_beta.is_file(), "{provider}: BetaSkill must deploy");
    assert!(
        deployed_helper.is_file(),
        "{provider}: BetaSkill/Helper.{ext} companion must deploy"
    );

    // Pass 2: drop BetaSkill from source, reinstall. Prune fires.
    fs::remove_dir_all(module.path().join("skills/BetaSkill")).unwrap();
    install(module.path(), target.path(), &[]).success();

    assert!(
        deployed_alpha.is_file(),
        "{provider}: AlphaSkill must survive"
    );
    assert!(
        !deployed_beta.exists(),
        "{provider}: BetaSkill/SKILL.{ext} must be pruned"
    );
    assert!(
        !deployed_helper.exists(),
        "{provider}: BetaSkill/Helper.{ext} companion must be pruned"
    );
    assert!(
        !target
            .path()
            .join(provider)
            .join("skills/BetaSkill")
            .exists(),
        "{provider}: BetaSkill directory must be removed (empty parent walk)"
    );

    let trash_entries = list_trash(target.path(), provider);
    assert_eq!(
        trash_entries.len(),
        1,
        "{provider}: exactly one timestamped trash entry expected, found {trash_entries:?}",
    );
    let trash_dir = &trash_entries[0];
    assert!(
        trash_dir
            .join(format!("skills/BetaSkill/SKILL.{ext}"))
            .is_file(),
        "{provider}: BetaSkill/SKILL.{ext} quarantined under {}",
        trash_dir.display()
    );
    assert!(
        trash_dir
            .join(format!("skills/BetaSkill/Helper.{ext}"))
            .is_file(),
        "{provider}: companion Helper.{ext} quarantined"
    );
}

#[test]
fn two_pass_prune_removes_skill_directory_claude() {
    run_two_pass_prune_for_provider(".claude");
}

#[test]
fn two_pass_prune_removes_skill_directory_gemini() {
    run_two_pass_prune_for_provider(".gemini");
}

#[test]
fn two_pass_prune_removes_skill_directory_codex() {
    run_two_pass_prune_for_provider(".codex");
}

#[test]
fn two_pass_prune_removes_skill_directory_opencode() {
    run_two_pass_prune_for_provider(".opencode");
}

// --- quarantine round-trip ---

#[test]
fn quarantine_roundtrip_restores_tree() {
    let module = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();

    scaffold_module_with_repo(
        module.path(),
        "rt-module",
        "https://github.com/example/rt-module",
    );
    create_skill(module.path(), "AlphaSkill");
    create_skill(module.path(), "BetaSkill");

    install(module.path(), target.path(), &[]).success();
    let pre_prune_skill =
        fs::read(target.path().join(".claude/skills/BetaSkill/SKILL.md")).unwrap();

    fs::remove_dir_all(module.path().join("skills/BetaSkill")).unwrap();
    install(module.path(), target.path(), &[]).success();

    // Locate the quarantined file.
    let trash_entries = list_trash(target.path(), ".claude");
    let quarantined = trash_entries[0].join("skills/BetaSkill/SKILL.md");
    let restored_content = fs::read(&quarantined).unwrap();
    assert_eq!(
        restored_content, pre_prune_skill,
        "quarantined SKILL.md must be byte-identical to pre-prune"
    );

    // Recovery contract: moving back restores the tree.
    fs::create_dir_all(target.path().join(".claude/skills/BetaSkill")).unwrap();
    fs::rename(
        &quarantined,
        target.path().join(".claude/skills/BetaSkill/SKILL.md"),
    )
    .unwrap();
    let post_restore = fs::read(target.path().join(".claude/skills/BetaSkill/SKILL.md")).unwrap();
    assert_eq!(post_restore, pre_prune_skill);
}

// --- flag behaviour ---

#[test]
fn no_prune_keeps_stale() {
    let module = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();

    scaffold_module_with_repo(
        module.path(),
        "np-module",
        "https://github.com/example/np-module",
    );
    create_skill(module.path(), "AlphaSkill");
    create_skill(module.path(), "BetaSkill");

    install(module.path(), target.path(), &[]).success();
    fs::remove_dir_all(module.path().join("skills/BetaSkill")).unwrap();
    install(module.path(), target.path(), &["--no-prune"]).success();

    assert!(
        target
            .path()
            .join(".claude/skills/BetaSkill/SKILL.md")
            .exists(),
        "BetaSkill must remain when --no-prune is passed"
    );
    assert!(
        list_trash(target.path(), ".claude").is_empty(),
        "no .trash/ should be created in --no-prune mode"
    );
}

#[test]
fn dry_run_emits_plan_without_moving() {
    let module = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();

    scaffold_module_with_repo(
        module.path(),
        "dr-module",
        "https://github.com/example/dr-module",
    );
    create_skill(module.path(), "AlphaSkill");
    create_skill(module.path(), "BetaSkill");

    install(module.path(), target.path(), &[]).success();
    fs::remove_dir_all(module.path().join("skills/BetaSkill")).unwrap();

    let output = install(module.path(), target.path(), &["--dry-run"]).success();
    let stderr = String::from_utf8_lossy(&output.get_output().stderr).into_owned();
    assert!(
        stderr.contains("would move"),
        "dry-run stderr must include 'would move', got:\n{stderr}"
    );
    assert!(
        stderr.contains("BetaSkill/SKILL.md"),
        "dry-run stderr must name BetaSkill, got:\n{stderr}"
    );
    assert!(
        target
            .path()
            .join(".claude/skills/BetaSkill/SKILL.md")
            .exists(),
        "dry-run must not move files"
    );
    assert!(
        list_trash(target.path(), ".claude").is_empty(),
        "dry-run must not create .trash/"
    );
}

// --- local-modification protection ---

#[test]
fn prune_skips_modified_file_without_force() {
    let module = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();

    scaffold_module_with_repo(
        module.path(),
        "mod-module",
        "https://github.com/example/mod-module",
    );
    create_skill(module.path(), "AlphaSkill");
    create_skill(module.path(), "BetaSkill");

    install(module.path(), target.path(), &[]).success();

    // User edits the deployed file in place.
    let deployed = target.path().join(".claude/skills/BetaSkill/SKILL.md");
    fs::write(&deployed, "user-modified content\n").unwrap();

    // Source removes BetaSkill; install with default prune.
    fs::remove_dir_all(module.path().join("skills/BetaSkill")).unwrap();
    install(module.path(), target.path(), &[]).success();

    assert!(
        deployed.is_file(),
        "modified file must survive prune without --force"
    );
    let content = fs::read_to_string(&deployed).unwrap();
    assert_eq!(
        content, "user-modified content\n",
        "modified content must be preserved verbatim"
    );
    assert!(
        list_trash(target.path(), ".claude").is_empty(),
        ".trash/ must not be created when only-modified files would be pruned"
    );
}

#[test]
fn prune_overrides_modified_file_with_force() {
    let module = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();

    scaffold_module_with_repo(
        module.path(),
        "force-module",
        "https://github.com/example/force-module",
    );
    create_skill(module.path(), "AlphaSkill");
    create_skill(module.path(), "BetaSkill");

    install(module.path(), target.path(), &[]).success();

    let deployed = target.path().join(".claude/skills/BetaSkill/SKILL.md");
    fs::write(&deployed, "user-modified content\n").unwrap();

    fs::remove_dir_all(module.path().join("skills/BetaSkill")).unwrap();
    install(module.path(), target.path(), &["--force"]).success();

    assert!(
        !deployed.exists(),
        "--force must let prune override the modification check"
    );
    let trash = list_trash(target.path(), ".claude");
    assert_eq!(trash.len(), 1, "exactly one .trash entry expected");
    let quarantined = trash[0].join("skills/BetaSkill/SKILL.md");
    let saved = fs::read_to_string(&quarantined).unwrap();
    assert_eq!(
        saved, "user-modified content\n",
        "quarantined copy must preserve the user's modifications"
    );
}

// --- hand-installed files are invisible to prune ---

#[test]
fn prune_ignores_files_not_in_manifest() {
    let module = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();

    scaffold_module_with_repo(
        module.path(),
        "hand-module",
        "https://github.com/example/hand-module",
    );
    create_skill(module.path(), "AlphaSkill");
    // GammaSkill kept across both passes so build/<provider>/skills/ stays
    // non-empty after AlphaSkill is removed; otherwise deploy::execute skips
    // the provider entirely and prune never runs.
    create_skill(module.path(), "GammaSkill");

    install(module.path(), target.path(), &[]).success();

    // Drop a hand-authored skill directly into the deployed tree; no
    // forge install was involved, so .manifest has no entry for it.
    let hand_skill_dir = target.path().join(".claude/skills/HandSkill");
    fs::create_dir_all(&hand_skill_dir).unwrap();
    let hand_file = hand_skill_dir.join("SKILL.md");
    fs::write(&hand_file, "---\nname: HandSkill\n---\n\nBody.\n").unwrap();

    // Reinstall (default prune). The hand-installed skill must survive
    // because the prune iterator only sees manifest entries.
    install(module.path(), target.path(), &[]).success();
    assert!(
        hand_file.is_file(),
        "hand-installed skill (no manifest entry) must be invisible to prune"
    );

    // Drop AlphaSkill from source; GammaSkill stays. Prune fires for
    // AlphaSkill, the hand file is still untouched.
    fs::remove_dir_all(module.path().join("skills/AlphaSkill")).unwrap();
    install(module.path(), target.path(), &[]).success();

    assert!(
        !target.path().join(".claude/skills/AlphaSkill").exists(),
        "AlphaSkill (with manifest entry) must be pruned"
    );
    assert!(
        hand_file.is_file(),
        "hand-installed skill must remain across the prune pass"
    );
    let saved = fs::read_to_string(&hand_file).unwrap();
    assert!(
        saved.contains("name: HandSkill"),
        "hand-installed content must be untouched"
    );
}

// --- rule removal (separate code branch from skills) ---

fn create_rule(root: &Path, name: &str) {
    let rules_dir = root.join("rules");
    fs::create_dir_all(&rules_dir).unwrap();
    fs::write(
        rules_dir.join(format!("{name}.md")),
        format!("Rule body for {name}.\n"),
    )
    .unwrap();
}

#[test]
fn prune_removes_stale_rule() {
    let module = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();

    scaffold_module_with_repo(
        module.path(),
        "rule-module",
        "https://github.com/example/rule-module",
    );
    create_rule(module.path(), "KeepThis");
    create_rule(module.path(), "DropThis");

    install(module.path(), target.path(), &[]).success();
    assert!(
        target.path().join(".claude/rules/DropThis.md").is_file(),
        "DropThis rule must deploy"
    );

    // Remove DropThis from source; reinstall.
    fs::remove_file(module.path().join("rules/DropThis.md")).unwrap();
    install(module.path(), target.path(), &[]).success();

    assert!(
        target.path().join(".claude/rules/KeepThis.md").is_file(),
        "KeepThis must survive"
    );
    assert!(
        !target.path().join(".claude/rules/DropThis.md").exists(),
        "DropThis must be pruned"
    );

    let trash = list_trash(target.path(), ".claude");
    assert_eq!(trash.len(), 1, "exactly one .trash entry expected");
    assert!(
        trash[0].join("rules/DropThis.md").is_file(),
        "DropThis quarantined under {}",
        trash[0].display()
    );
}

// --- bare-name module identity ---

#[test]
fn prune_handles_bare_name_modules() {
    let module = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();

    scaffold_module_bare(module.path(), "bare-module");
    create_skill(module.path(), "AlphaSkill");
    create_skill(module.path(), "BetaSkill");

    install(module.path(), target.path(), &[]).success();
    fs::remove_dir_all(module.path().join("skills/BetaSkill")).unwrap();
    install(module.path(), target.path(), &[]).success();

    assert!(
        !target.path().join(".claude/skills/BetaSkill").exists(),
        "BetaSkill must be pruned even when module identity is a bare name"
    );
}
