//! Integration test for `forge drift --target`: verify a module's assembled
//! build against where it was deployed, scoped to the module's own files.

use assert_cmd::Command;
use std::fs;
use std::path::Path;

fn forge() -> Command {
    Command::cargo_bin("forge").unwrap()
}

fn scaffold_module(root: &Path) {
    fs::write(
        root.join("module.yaml"),
        "name: drift-fixture\nversion: 0.1.0\ndescription: drift target fixture\nevents: []\nrepository: https://github.com/example/drift-fixture\n",
    )
    .unwrap();
    fs::write(root.join("defaults.yaml"), "").unwrap();
    let rules = root.join("rules");
    fs::create_dir_all(&rules).unwrap();
    fs::write(
        rules.join("KeepDeployed.md"),
        "---\nname: KeepDeployed\ndescription: a rule\n---\n\nOriginal body.\n",
    )
    .unwrap();
}

fn install(module: &Path, target: &Path) {
    forge()
        .args([
            "install",
            "--source",
            module.to_str().unwrap(),
            "--target",
            target.to_str().unwrap(),
        ])
        .assert()
        .success();
}

#[test]
fn drift_target_clean_after_install() {
    let module = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    scaffold_module(module.path());
    install(module.path(), target.path());

    forge()
        .args([
            "drift",
            "--source",
            module.path().to_str().unwrap(),
            "--target",
            target.path().to_str().unwrap(),
        ])
        .assert()
        .success();
}

#[test]
fn drift_target_detects_edited_deployment() {
    let module = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    scaffold_module(module.path());
    install(module.path(), target.path());

    // Edit the deployed rule out from under the build.
    let deployed_rule = target.path().join(".claude/rules/KeepDeployed.md");
    let edited = fs::read_to_string(&deployed_rule).unwrap() + "\nUser appended a line.\n";
    fs::write(&deployed_rule, edited).unwrap();

    forge()
        .args([
            "drift",
            "--source",
            module.path().to_str().unwrap(),
            "--target",
            target.path().to_str().unwrap(),
        ])
        .assert()
        .failure();
}

#[test]
fn drift_target_ignores_other_modules_in_deployment() {
    let module = tempfile::tempdir().unwrap();
    let target = tempfile::tempdir().unwrap();
    scaffold_module(module.path());
    install(module.path(), target.path());

    // A foreign file with no provenance attribution to this module. Scoped
    // verify must not flag it; the result stays clean.
    fs::write(
        target.path().join(".claude/rules/Foreign.md"),
        "---\nname: Foreign\n---\nfrom another module\n",
    )
    .unwrap();

    forge()
        .args([
            "drift",
            "--source",
            module.path().to_str().unwrap(),
            "--target",
            target.path().to_str().unwrap(),
        ])
        .assert()
        .success();
}

#[test]
fn drift_requires_a_mode() {
    let module = tempfile::tempdir().unwrap();
    scaffold_module(module.path());
    forge()
        .args(["drift", "--source", module.path().to_str().unwrap()])
        .assert()
        .failure();
}
