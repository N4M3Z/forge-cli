//! Integration tests for `.forge` git-source resolution.
//!
//! The fixture pattern: create a bare git repo on the local filesystem,
//! commit a small module into it, then point a consumer `.forge` at the
//! bare repo via `file://` URL (test-only escape hatch). The runtime uses
//! gix to clone + materialize the pinned SHA into a tempdir cache, runs
//! the standard assemble + deploy pipeline, and asserts the artifact
//! lands in the consumer's provider trees.

use assert_cmd::Command;
use std::fs;
use std::path::Path;
use std::process::Command as StdCommand;

fn forge() -> Command {
    Command::cargo_bin("forge").unwrap()
}

fn git(args: &[&str], cwd: &Path) -> std::process::Output {
    // The test fixtures must never inherit the developer's commit.gpgsign /
    // tag.gpgsign settings — those trigger YubiKey touch prompts in CI and
    // serialize parallel tests. -c overrides win over user / system config.
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

fn scaffold_producer_in_workdir(work: &Path) {
    fs::write(
        work.join("module.yaml"),
        "name: producer\nversion: 0.1.0\ndescription: git fixture\nevents: []\nrepository: https://example.com/producer\n",
    )
    .unwrap();
    fs::write(work.join("defaults.yaml"), "").unwrap();
    let skill_dir = work.join("skills").join("GitSkill");
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(
        skill_dir.join("SKILL.md"),
        "---\nname: GitSkill\ndescription: fixture skill in a git repo\nversion: 0.1.0\n---\n\nBody.\n",
    )
    .unwrap();
}

/// Create a bare git repo at `bare_path`, populate it with a module via a
/// scratch worktree, push, and return the resulting commit SHA.
fn make_fixture_repo(bare_path: &Path, scratch: &Path) -> String {
    git(&["init", "--bare", bare_path.to_str().unwrap()], scratch);
    git(
        &["init", "--initial-branch=main", scratch.to_str().unwrap()],
        scratch,
    );
    scaffold_producer_in_workdir(scratch);
    git(&["add", "."], scratch);
    git(&["commit", "-m", "fixture commit"], scratch);
    git(
        &[
            "remote",
            "add",
            "origin",
            &format!("file://{}", bare_path.display()),
        ],
        scratch,
    );
    git(&["push", "origin", "main"], scratch);
    let sha = git(&["rev-parse", "HEAD"], scratch);
    String::from_utf8(sha.stdout).unwrap().trim().to_string()
}

#[test]
fn dotforge_git_source_clones_and_deploys_pinned_sha() {
    let bare = tempfile::tempdir().unwrap();
    let scratch = tempfile::tempdir().unwrap();
    let consumer = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();

    let bare_path = bare.path().join("producer.git");
    let sha = make_fixture_repo(&bare_path, scratch.path());

    fs::write(
        consumer.path().join(".forge"),
        format!(
            "version: 1\n\
             sources:\n  \
                producer:\n    \
                    git: file://{bare}\n    \
                    ref: {sha}\n\
             artifacts:\n  \
                producer:\n    \
                    skills: [GitSkill]\n",
            bare = bare_path.display(),
            sha = sha,
        ),
    )
    .unwrap();

    forge()
        .args(["install", "--source", consumer.path().to_str().unwrap()])
        .env("FORGE_GIT_ALLOW_FILE_URLS", "1")
        .env("FORGE_GIT_CACHE_DIR", cache.path())
        .assert()
        .success();

    assert!(
        consumer
            .path()
            .join(".claude/skills/GitSkill/SKILL.md")
            .is_file(),
        "consumer/.claude must receive GitSkill from the git source"
    );
}

#[test]
fn dotforge_git_source_rejects_uncached_sha_without_match() {
    let bare = tempfile::tempdir().unwrap();
    let scratch = tempfile::tempdir().unwrap();
    let consumer = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();

    let bare_path = bare.path().join("producer.git");
    make_fixture_repo(&bare_path, scratch.path());

    let bogus_sha = "0123456789abcdef0123456789abcdef01234567";
    fs::write(
        consumer.path().join(".forge"),
        format!(
            "version: 1\n\
             sources:\n  \
                producer:\n    \
                    git: file://{bare}\n    \
                    ref: {bogus_sha}\n\
             artifacts:\n  \
                producer:\n    \
                    skills: [GitSkill]\n",
            bare = bare_path.display(),
        ),
    )
    .unwrap();

    let output = forge()
        .args(["install", "--source", consumer.path().to_str().unwrap()])
        .env("FORGE_GIT_ALLOW_FILE_URLS", "1")
        .env("FORGE_GIT_CACHE_DIR", cache.path())
        .assert()
        .failure();
    let stderr = String::from_utf8_lossy(&output.get_output().stderr).into_owned();
    assert!(
        stderr.contains(bogus_sha) && stderr.to_lowercase().contains("not found"),
        "error must name the missing SHA and explain not-found: {stderr}"
    );
}

#[test]
fn dotforge_git_source_is_cache_idempotent() {
    let bare = tempfile::tempdir().unwrap();
    let scratch = tempfile::tempdir().unwrap();
    let consumer = tempfile::tempdir().unwrap();
    let cache = tempfile::tempdir().unwrap();

    let bare_path = bare.path().join("producer.git");
    let sha = make_fixture_repo(&bare_path, scratch.path());

    let manifest = format!(
        "version: 1\n\
         sources:\n  \
            producer:\n    \
                git: file://{bare}\n    \
                ref: {sha}\n\
         artifacts:\n  \
            producer:\n    \
                skills: [GitSkill]\n",
        bare = bare_path.display(),
    );
    fs::write(consumer.path().join(".forge"), &manifest).unwrap();

    forge()
        .args(["install", "--source", consumer.path().to_str().unwrap()])
        .env("FORGE_GIT_ALLOW_FILE_URLS", "1")
        .env("FORGE_GIT_CACHE_DIR", cache.path())
        .assert()
        .success();
    let first = fs::read(consumer.path().join(".claude/.manifest")).unwrap();

    forge()
        .args(["install", "--source", consumer.path().to_str().unwrap()])
        .env("FORGE_GIT_ALLOW_FILE_URLS", "1")
        .env("FORGE_GIT_CACHE_DIR", cache.path())
        .assert()
        .success();
    let second = fs::read(consumer.path().join(".claude/.manifest")).unwrap();

    assert_eq!(
        first, second,
        ".manifest must be byte-identical across runs"
    );
}
