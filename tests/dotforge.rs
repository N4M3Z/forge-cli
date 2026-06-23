//! Integration tests for the `.forge` consumer manifest.
//!
//! A consumer repo declares which artifacts it wants in `.forge`; `forge
//! install` from that repo reads the manifest, walks the named producer
//! modules on disk, and deploys only the requested subset.

use assert_cmd::Command;
use std::fs;
use std::path::Path;

fn forge() -> Command {
    Command::cargo_bin("forge").unwrap()
}

fn scaffold_producer(root: &Path, name: &str) {
    fs::write(
        root.join("module.yaml"),
        format!(
            "name: {name}\nversion: 0.1.0\ndescription: test producer\nevents: []\nrepository: https://github.com/example/{name}\n"
        ),
    )
    .unwrap();
    fs::write(root.join("defaults.yaml"), "").unwrap();
}

fn write_skill(producer_root: &Path, name: &str) {
    let dir = producer_root.join("skills").join(name);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("SKILL.md"),
        format!(
            "---\nname: {name}\ndescription: integration fixture skill\nversion: 0.1.0\n---\n\nBody for {name}.\n"
        ),
    )
    .unwrap();
}

fn write_rule(producer_root: &Path, name: &str) {
    let dir = producer_root.join("rules");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join(format!("{name}.md")),
        format!("Rule body for {name}.\n"),
    )
    .unwrap();
}

fn write_dotforge(consumer_root: &Path, body: &str) {
    fs::write(consumer_root.join(".forge"), body).unwrap();
}

fn install(consumer_root: &Path) -> assert_cmd::assert::Assert {
    forge()
        .args([
            "install",
            "--source",
            consumer_root.to_str().unwrap(),
            "--target",
            consumer_root.to_str().unwrap(),
        ])
        .assert()
}

#[test]
fn dotforge_deploys_requested_artifacts_across_providers() {
    let producer_a = tempfile::tempdir().unwrap();
    let producer_b = tempfile::tempdir().unwrap();
    let consumer = tempfile::tempdir().unwrap();

    scaffold_producer(producer_a.path(), "producer-a");
    write_skill(producer_a.path(), "AlphaSkill");
    write_skill(producer_a.path(), "UnreqSkill");

    scaffold_producer(producer_b.path(), "producer-b");
    write_rule(producer_b.path(), "KeepThis");
    write_rule(producer_b.path(), "Unrequested");

    write_dotforge(
        consumer.path(),
        &format!(
            "version: 1\n\
             sources:\n  \
                producer-a:\n    \
                    path: {a}\n  \
                producer-b:\n    \
                    path: {b}\n\
             artifacts:\n  \
                producer-a:\n    \
                    skills: [AlphaSkill]\n  \
                producer-b:\n    \
                    rules: [KeepThis]\n",
            a = producer_a.path().display(),
            b = producer_b.path().display(),
        ),
    );

    install(consumer.path()).success();

    for provider in [".claude", ".gemini", ".opencode"] {
        assert!(
            consumer
                .path()
                .join(provider)
                .join("skills/AlphaSkill/SKILL.md")
                .is_file(),
            "{provider}: AlphaSkill must deploy"
        );
        assert!(
            consumer
                .path()
                .join(provider)
                .join("rules/KeepThis.md")
                .is_file(),
            "{provider}: KeepThis rule must deploy"
        );
        assert!(
            !consumer
                .path()
                .join(provider)
                .join("skills/UnreqSkill")
                .exists(),
            "{provider}: unrequested skill must not deploy"
        );
        assert!(
            !consumer
                .path()
                .join(provider)
                .join("rules/Unrequested.md")
                .exists(),
            "{provider}: unrequested rule must not deploy"
        );
    }
    // Codex skills deploy as SKILL.md; agents-to-toml converts agents only.
    assert!(
        consumer
            .path()
            .join(".codex/skills/AlphaSkill/SKILL.md")
            .is_file(),
        "codex: AlphaSkill must deploy as SKILL.md"
    );
}

#[test]
fn dotforge_errors_on_missing_source_path() {
    let consumer = tempfile::tempdir().unwrap();
    write_dotforge(
        consumer.path(),
        "version: 1\nsources:\n  ghost:\n    path: /definitely/does/not/exist\nartifacts:\n  ghost:\n    skills: [X]\n",
    );

    let output = install(consumer.path()).failure();
    let stderr = String::from_utf8_lossy(&output.get_output().stderr).into_owned();
    assert!(
        stderr.contains("ghost") && stderr.contains("does not exist"),
        "error must name the missing source: {stderr}"
    );
}

#[test]
fn dotforge_errors_on_missing_artifact_in_source() {
    let producer = tempfile::tempdir().unwrap();
    let consumer = tempfile::tempdir().unwrap();
    scaffold_producer(producer.path(), "producer");
    write_skill(producer.path(), "RealSkill");

    write_dotforge(
        consumer.path(),
        &format!(
            "version: 1\nsources:\n  producer:\n    path: {p}\nartifacts:\n  producer:\n    skills: [DoesNotExist]\n",
            p = producer.path().display(),
        ),
    );

    let output = install(consumer.path()).failure();
    let stderr = String::from_utf8_lossy(&output.get_output().stderr).into_owned();
    assert!(
        stderr.contains("DoesNotExist") && stderr.contains("not found"),
        "error must name the missing artifact: {stderr}"
    );
}

#[test]
fn dotforge_install_is_idempotent() {
    let producer = tempfile::tempdir().unwrap();
    let consumer = tempfile::tempdir().unwrap();
    scaffold_producer(producer.path(), "producer");
    write_skill(producer.path(), "AlphaSkill");

    write_dotforge(
        consumer.path(),
        &format!(
            "version: 1\nsources:\n  producer:\n    path: {p}\nartifacts:\n  producer:\n    skills: [AlphaSkill]\n",
            p = producer.path().display(),
        ),
    );

    install(consumer.path()).success();
    let first_manifest = fs::read(consumer.path().join(".claude/.manifest")).unwrap();
    let first_skill = fs::read(consumer.path().join(".claude/skills/AlphaSkill/SKILL.md")).unwrap();

    install(consumer.path()).success();
    let second_manifest = fs::read(consumer.path().join(".claude/.manifest")).unwrap();
    let second_skill =
        fs::read(consumer.path().join(".claude/skills/AlphaSkill/SKILL.md")).unwrap();

    assert_eq!(
        first_manifest, second_manifest,
        ".manifest must be byte-identical across runs"
    );
    assert_eq!(
        first_skill, second_skill,
        "deployed skill must be byte-identical across runs"
    );
}

#[test]
fn dotforge_defaults_target_to_source_when_omitted() {
    let producer = tempfile::tempdir().unwrap();
    let consumer = tempfile::tempdir().unwrap();
    scaffold_producer(producer.path(), "producer");
    write_skill(producer.path(), "AlphaSkill");

    write_dotforge(
        consumer.path(),
        &format!(
            "version: 1\nsources:\n  producer:\n    path: {p}\nartifacts:\n  producer:\n    skills: [AlphaSkill]\n",
            p = producer.path().display(),
        ),
    );

    // Note: no --target. Issue #52 says deploying should land under the consumer dir.
    forge()
        .args(["install", "--source", consumer.path().to_str().unwrap()])
        .assert()
        .success();

    assert!(
        consumer
            .path()
            .join(".claude/skills/AlphaSkill/SKILL.md")
            .is_file(),
        "consumer/.claude must receive AlphaSkill when --target is omitted"
    );
    assert!(
        consumer
            .path()
            .join(".gemini/skills/AlphaSkill/SKILL.md")
            .is_file(),
        "consumer/.gemini must receive AlphaSkill when --target is omitted"
    );
}

#[test]
fn dotforge_errors_on_oversized_file() {
    let consumer = tempfile::tempdir().unwrap();
    // 65 KiB of payload — over the 64 KiB cap enforced in dotforge::load.
    let payload = "x".repeat(65 * 1024);
    fs::write(consumer.path().join(".forge"), payload).unwrap();

    let output = install(consumer.path()).failure();
    let stderr = String::from_utf8_lossy(&output.get_output().stderr).into_owned();
    assert!(
        stderr.contains("limit is") && stderr.contains("bytes"),
        "error must name the size cap: {stderr}"
    );
}
