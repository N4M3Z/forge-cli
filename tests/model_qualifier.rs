//! Integration tests for model-level qualifier resolution (#60, Phase 1).
//!
//! Resolution precedence is `user/` > `provider/<model>/` > `provider/` >
//! base. A file placed in a model qualifier directory must override the base
//! for the matching provider+model and must not silently disappear.

use assert_cmd::Command;
use std::fs;
use std::path::Path;

fn forge() -> Command {
    Command::cargo_bin("forge").unwrap()
}

fn scaffold(root: &Path) {
    fs::write(
        root.join("module.yaml"),
        "name: model-fixture\nversion: 0.1.0\ndescription: model qualifier fixture\nevents: []\n",
    )
    .unwrap();
    fs::write(root.join("defaults.yaml"), "").unwrap();
}

fn write_rule(root: &Path, relative: &str, body: &str) {
    let path = root.join("rules").join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(
        path,
        format!("---\nname: Base\ndescription: r\n---\n\n{body}\n"),
    )
    .unwrap();
}

fn assemble(module: &Path) {
    forge()
        .args(["assemble", "--source", module.to_str().unwrap()])
        .assert()
        .success();
}

#[test]
fn model_variant_overrides_base_for_matching_model() {
    let module = tempfile::tempdir().unwrap();
    scaffold(module.path());
    write_rule(module.path(), "Base.md", "BASE BODY");
    // claude's default model is claude-opus-4-6 (from embedded defaults).
    write_rule(
        module.path(),
        "claude/claude-opus-4-6/Base.md",
        "OPUS VARIANT BODY",
    );

    assemble(module.path());

    let claude = fs::read_to_string(module.path().join("build/claude/rules/Base.md")).unwrap();
    assert!(
        claude.contains("OPUS VARIANT BODY"),
        "claude (model claude-opus-4-6) must get the model variant: {claude}"
    );

    // gemini's default model is gemini-2.5-pro, so the claude-opus variant
    // must not leak into the gemini build.
    let gemini = fs::read_to_string(module.path().join("build/gemini/rules/Base.md")).unwrap();
    assert!(
        gemini.contains("BASE BODY"),
        "gemini must fall back to the base body: {gemini}"
    );
}

#[test]
fn user_overlay_wins_over_model_variant() {
    let module = tempfile::tempdir().unwrap();
    scaffold(module.path());
    write_rule(module.path(), "Base.md", "BASE BODY");
    write_rule(
        module.path(),
        "claude/claude-opus-4-6/Base.md",
        "OPUS VARIANT BODY",
    );
    write_rule(module.path(), "user/Base.md", "USER OVERLAY BODY");

    assemble(module.path());

    let claude = fs::read_to_string(module.path().join("build/claude/rules/Base.md")).unwrap();
    assert!(
        claude.contains("USER OVERLAY BODY"),
        "user/ overlay must win over the model variant: {claude}"
    );
}

#[test]
fn model_only_file_is_deployed_not_dropped() {
    let module = tempfile::tempdir().unwrap();
    scaffold(module.path());
    write_rule(module.path(), "Base.md", "BASE BODY");
    write_rule(
        module.path(),
        "claude/claude-opus-4-6/OpusOnly.md",
        "OPUS ONLY BODY",
    );

    assemble(module.path());

    assert!(
        module
            .path()
            .join("build/claude/rules/OpusOnly.md")
            .is_file(),
        "a model-only file must deploy for its provider, not silently vanish"
    );
    assert!(
        !module
            .path()
            .join("build/gemini/rules/OpusOnly.md")
            .exists(),
        "a claude model-only file must not deploy for gemini"
    );
}

#[test]
fn model_override_flag_selects_variant() {
    let module = tempfile::tempdir().unwrap();
    scaffold(module.path());
    write_rule(module.path(), "Base.md", "BASE BODY");
    write_rule(
        module.path(),
        "claude/claude-sonnet-4-6/Base.md",
        "SONNET VARIANT BODY",
    );

    // Default model is claude-opus-4-6, so without --model the sonnet variant
    // is not selected; with --model claude-sonnet-4-6 it wins.
    forge()
        .args([
            "assemble",
            "--source",
            module.path().to_str().unwrap(),
            "--model",
            "claude-sonnet-4-6",
        ])
        .assert()
        .success();

    let claude = fs::read_to_string(module.path().join("build/claude/rules/Base.md")).unwrap();
    assert!(
        claude.contains("SONNET VARIANT BODY"),
        "--model claude-sonnet-4-6 must select the sonnet variant: {claude}"
    );
}

#[test]
fn unknown_subdirectory_in_qualifier_dir_warns_and_skips() {
    let module = tempfile::tempdir().unwrap();
    scaffold(module.path());
    write_rule(module.path(), "Base.md", "BASE BODY");
    // A non-model subdirectory inside the claude qualifier directory. Phase 1
    // warns and skips it (rather than silently dropping or hard-failing).
    fs::create_dir_all(module.path().join("rules/claude/not-a-model")).unwrap();
    fs::write(
        module.path().join("rules/claude/not-a-model/Stray.md"),
        "stray\n",
    )
    .unwrap();

    let output = forge()
        .args(["assemble", "--source", module.path().to_str().unwrap()])
        .assert()
        .success();
    let stderr = String::from_utf8_lossy(&output.get_output().stderr).into_owned();
    assert!(
        stderr.contains("not-a-model"),
        "the skipped directory must be named in a warning: {stderr}"
    );
    assert!(
        !module.path().join("build/claude/rules/Stray.md").exists(),
        "the stray file must not be deployed"
    );
}
