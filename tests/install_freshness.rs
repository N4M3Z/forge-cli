//! Integration tests for `forge install` source git freshness checks.

use assert_cmd::Command;
use std::fs;
use std::path::Path;
use std::process::Command as StdCommand;

fn forge() -> Command {
    Command::cargo_bin("forge").unwrap()
}

fn git(args: &[&str], cwd: &Path) -> std::process::Output {
    let mut full_args: Vec<&str> = vec![
        "-c",
        "commit.gpgsign=false",
        "-c",
        "tag.gpgsign=false",
        "-c",
        "gpg.format=openpgp",
    ];
    full_args.extend_from_slice(args);
    let output = StdCommand::new("git")
        .args(&full_args)
        .current_dir(cwd)
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_DIR")
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@example.com")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@example.com")
        .output()
        .expect("git command must run");
    assert!(
        output.status.success(),
        "git {args:?} in {} failed: stdout={} stderr={}",
        cwd.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    output
}

fn scaffold_module(root: &Path) {
    fs::write(
        root.join("module.yaml"),
        "name: freshness-fixture\nversion: 0.1.0\ndescription: freshness fixture\nevents: []\n",
    )
    .unwrap();
    fs::write(root.join("defaults.yaml"), "").unwrap();
    let rules = root.join("rules");
    fs::create_dir_all(&rules).unwrap();
    fs::write(rules.join("Freshness.md"), "base rule\n").unwrap();
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

fn init_git_module(root: &Path) {
    git(&["init", "--initial-branch=main"], root);
    scaffold_module(root);
    git(&["add", "."], root);
    git(&["commit", "-m", "base"], root);
}

fn head_sha(root: &Path) -> String {
    let output = git(&["rev-parse", "HEAD"], root);
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

fn make_origin_main_match_head(root: &Path) {
    let head = head_sha(root);
    git(&["update-ref", "refs/remotes/origin/main", &head], root);
}

fn make_origin_main_ahead(root: &Path) {
    let base = head_sha(root);
    fs::write(root.join("rules/Freshness.md"), "trunk rule\n").unwrap();
    git(&["add", "."], root);
    git(&["commit", "-m", "trunk"], root);
    let trunk = head_sha(root);
    git(&["update-ref", "refs/remotes/origin/main", &trunk], root);
    git(&["reset", "--hard", &base], root);
}

#[test]
fn stale_git_source_refuses_install_without_allow_stale() {
    let module = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    init_git_module(module.path());
    make_origin_main_ahead(module.path());

    let output = install(module.path(), target.path(), &[]).failure();
    let stderr = String::from_utf8_lossy(&output.get_output().stderr).into_owned();

    assert!(
        stderr.contains("WARNING"),
        "stale install must print a prominent warning: {stderr}"
    );
    assert!(
        stderr.contains("1 commit behind refs/remotes/origin/main"),
        "warning must name how far behind origin/main it is: {stderr}"
    );
    assert!(
        stderr.contains("--allow-stale"),
        "refusal must explain the override flag: {stderr}"
    );
    assert!(
        !target.path().join(".claude/rules/Freshness.md").exists(),
        "refused install must not deploy stale content"
    );
}

#[test]
fn stale_git_source_allows_install_with_allow_stale() {
    let module = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    init_git_module(module.path());
    make_origin_main_ahead(module.path());

    let output = install(module.path(), target.path(), &["--allow-stale"]).success();
    let stderr = String::from_utf8_lossy(&output.get_output().stderr).into_owned();

    assert!(
        stderr.contains("WARNING"),
        "--allow-stale must still show the freshness warning: {stderr}"
    );
    assert!(
        target.path().join(".claude/rules/Freshness.md").is_file(),
        "--allow-stale should continue into deployment"
    );
}

#[test]
fn non_git_source_skips_freshness_check_silently() {
    let module = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    scaffold_module(module.path());

    let output = install(module.path(), target.path(), &[]).success();
    let stderr = String::from_utf8_lossy(&output.get_output().stderr).into_owned();

    assert!(
        !stderr.contains("freshness") && !stderr.contains("WARNING"),
        "non-git sources should not warn about freshness: {stderr}"
    );
}

#[test]
fn up_to_date_git_source_does_not_warn() {
    let module = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    init_git_module(module.path());
    make_origin_main_match_head(module.path());

    let output = install(module.path(), target.path(), &[]).success();
    let stderr = String::from_utf8_lossy(&output.get_output().stderr).into_owned();

    assert!(
        !stderr.contains("behind") && !stderr.contains("WARNING"),
        "up-to-date source should not warn: {stderr}"
    );
}
