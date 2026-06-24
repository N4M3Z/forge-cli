//! Integration tests for source-side `forge provenance`.
//!
//! A source repository carries `.provenance/*.yaml` sidecars next to the
//! artifacts they describe. `forge provenance --target <repo|subdir>` walks
//! those sidecars, resolves each subject back to a repo-relative file, and
//! recomputes its SHA-256 (plus any in-repo dependency digests).

use assert_cmd::Command;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

fn forge() -> Command {
    Command::cargo_bin("forge").unwrap()
}

fn sha256(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn scaffold_adopted_skill(repo: &Path, body: &str, subject_digest: &str) {
    fs::write(repo.join("module.yaml"), "name: fixture\n").unwrap();
    let skill_dir = repo.join("skills/Adopted");
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(skill_dir.join("SKILL.md"), body).unwrap();

    let provenance_dir = skill_dir.join(".provenance");
    fs::create_dir_all(&provenance_dir).unwrap();
    let yaml = format!(
        "provenance:\n    _type: https://in-toto.io/Statement/v1\n    subject:\n        - name: skills/Adopted/SKILL.md\n          digest:\n              sha256: {subject_digest}\n    predicate:\n        buildDefinition:\n            buildType: https://forge-cli/adopt/v1\n            externalParameters:\n                upstream_url: https://example.test/upstream\n            resolvedDependencies:\n                - name: upstream\n                  uri: https://example.test/upstream\n                  digest:\n                      sha256: deadbeef\n"
    );
    fs::write(provenance_dir.join("SKILL.yaml"), yaml).unwrap();
}

#[test]
fn provenance_source_verifies_matching_subject() {
    let repo = tempfile::tempdir().unwrap();
    let body = "Adopted skill body.\n";
    scaffold_adopted_skill(repo.path(), body, &sha256(body));

    forge()
        .args(["provenance", "--target", repo.path().to_str().unwrap()])
        .assert()
        .success();
}

#[test]
fn provenance_source_fails_on_stale_subject() {
    let repo = tempfile::tempdir().unwrap();
    // Sidecar records the digest of the original body; the file on disk differs.
    scaffold_adopted_skill(
        repo.path(),
        "edited after adoption\n",
        &sha256("original\n"),
    );

    forge()
        .args(["provenance", "--target", repo.path().to_str().unwrap()])
        .assert()
        .failure();
}

#[test]
fn provenance_source_scopes_to_subdirectory() {
    let repo = tempfile::tempdir().unwrap();
    let body = "Adopted skill body.\n";
    scaffold_adopted_skill(repo.path(), body, &sha256(body));

    forge()
        .args([
            "provenance",
            "--target",
            repo.path().join("skills/Adopted").to_str().unwrap(),
        ])
        .assert()
        .success();
}

#[test]
fn provenance_source_json_reports_subject_and_source() {
    let repo = tempfile::tempdir().unwrap();
    let body = "Adopted skill body.\n";
    scaffold_adopted_skill(repo.path(), body, &sha256(body));

    let output = forge()
        .args([
            "provenance",
            "--target",
            repo.path().to_str().unwrap(),
            "--json",
        ])
        .assert()
        .success();
    let stdout = String::from_utf8_lossy(&output.get_output().stdout).into_owned();
    assert!(
        stdout.contains("\"subject\": \"skills/Adopted/SKILL.md\""),
        "json must name the subject: {stdout}"
    );
    assert!(
        stdout.contains("https://example.test/upstream"),
        "json must carry the resolved source: {stdout}"
    );
}
