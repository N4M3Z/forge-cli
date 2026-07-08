use super::super::{DriftResult, DriftStatus};
use super::*;
use commands::manifest;
use tempfile::TempDir;

fn write(path: &std::path::Path, content: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
}

/// Deploy a file plus its `.manifest` entry and a provenance sidecar that
/// attributes it to `source_uri`, mimicking a real `forge install`.
fn deploy_owned_file(
    deployed_base: &std::path::Path,
    relative: &str,
    content: &str,
    source_uri: &str,
) {
    use std::fmt::Write as _;
    write(&deployed_base.join(relative), content);

    let artifact = std::path::Path::new(relative);
    let stem = artifact.file_stem().unwrap().to_string_lossy();
    let provenance_relative = format!(
        "{}/.provenance/{stem}.yaml",
        artifact.parent().unwrap().to_string_lossy()
    );
    let sidecar = format!(
        "provenance:\n    _type: https://in-toto.io/Statement/v1\n    subject:\n        - name: {relative}\n          digest:\n              sha256: {digest}\n    predicate:\n        buildDefinition:\n            buildType: https://forge-cli/assemble/v1\n            externalParameters:\n                source: {source_uri}\n            resolvedDependencies: []\n        runDetails:\n            builder:\n                id: forge-cli\n                version:\n                    forge: 0.0.0\n            metadata:\n                startedOn: \"2026-01-01T00:00:00Z\"\n",
        digest = manifest::content_sha256(content)
    );
    write(&deployed_base.join(&provenance_relative), &sidecar);

    let manifest_path = deployed_base.join(".manifest");
    let mut manifest_yaml = std::fs::read_to_string(&manifest_path).unwrap_or_default();
    let digest = manifest::content_sha256(content);
    let _ = write!(
        manifest_yaml,
        "{relative}:\n    fingerprint: {digest}\n    provenance: {provenance_relative}\n"
    );
    std::fs::write(&manifest_path, manifest_yaml).unwrap();
}

fn run(build: &std::path::Path, deployed: &std::path::Path, module: &str) -> DriftResult {
    let mut result = DriftResult::default();
    compare_provider(
        &mut result,
        build,
        deployed,
        "claude",
        Some(module),
        &HashSet::new(),
    );
    result
}

const MODULE: &str = "https://github.com/example/mymod";

#[test]
fn identical_build_and_deployed_is_not_drift() {
    let build = TempDir::new().unwrap();
    let deployed = TempDir::new().unwrap();
    let body = "---\nname: Foo\n---\nBody.\n";
    write(&build.path().join("rules/Foo.md"), body);
    deploy_owned_file(deployed.path(), "rules/Foo.md", body, MODULE);

    let result = run(build.path(), deployed.path(), MODULE);
    let entry = result
        .entries
        .iter()
        .find(|e| e.name == "rules/Foo.md")
        .unwrap();
    assert_eq!(entry.status, DriftStatus::Identical);
}

#[test]
fn edited_deployment_reports_body_drift() {
    let build = TempDir::new().unwrap();
    let deployed = TempDir::new().unwrap();
    write(
        &build.path().join("rules/Foo.md"),
        "---\nname: Foo\n---\nOriginal.\n",
    );
    deploy_owned_file(
        deployed.path(),
        "rules/Foo.md",
        "---\nname: Foo\n---\nUser edited this.\n",
        MODULE,
    );

    let result = run(build.path(), deployed.path(), MODULE);
    let entry = result
        .entries
        .iter()
        .find(|e| e.name == "rules/Foo.md")
        .unwrap();
    assert_eq!(entry.status, DriftStatus::BodyOnly);
}

#[test]
fn built_but_not_deployed_is_local_only() {
    let build = TempDir::new().unwrap();
    let deployed = TempDir::new().unwrap();
    write(
        &build.path().join("rules/New.md"),
        "---\nname: New\n---\nBody.\n",
    );

    let result = run(build.path(), deployed.path(), MODULE);
    let entry = result
        .entries
        .iter()
        .find(|e| e.name == "rules/New.md")
        .unwrap();
    assert_eq!(entry.status, DriftStatus::LocalOnly);
}

#[test]
fn deployed_but_not_built_owned_is_drift() {
    let build = TempDir::new().unwrap();
    let deployed = TempDir::new().unwrap();
    // Module deployed a file previously; the current build no longer contains it.
    deploy_owned_file(deployed.path(), "rules/Removed.md", "old body\n", MODULE);

    let result = run(build.path(), deployed.path(), MODULE);
    let entry = result
        .entries
        .iter()
        .find(|e| e.name == "rules/Removed.md")
        .expect("stale deployed file must be reported");
    assert_eq!(entry.status, DriftStatus::UpstreamOnly);
}

#[test]
fn other_modules_deployed_files_are_ignored() {
    let build = TempDir::new().unwrap();
    let deployed = TempDir::new().unwrap();
    // A file deployed by a DIFFERENT module must not appear in this module's report.
    deploy_owned_file(
        deployed.path(),
        "rules/Foreign.md",
        "foreign body\n",
        "https://github.com/other/theirmod",
    );

    let result = run(build.path(), deployed.path(), MODULE);
    assert!(
        result.entries.iter().all(|e| e.name != "rules/Foreign.md"),
        "another module's file must be out of scope: {:?}",
        result.entries.iter().map(|e| &e.name).collect::<Vec<_>>()
    );
}
