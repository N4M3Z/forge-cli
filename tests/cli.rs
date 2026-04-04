use assert_cmd::Command;
use predicates::prelude::*;

fn forge() -> Command {
    Command::cargo_bin("forge").unwrap()
}

#[test]
fn version_flag_prints_version() {
    forge()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("forge"));
}

#[test]
fn help_flag_lists_subcommands() {
    forge()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("install"))
        .stdout(predicate::str::contains("assemble"))
        .stdout(predicate::str::contains("copy"))
        .stdout(predicate::str::contains("validate"))
        .stdout(predicate::str::contains("release"));
}

#[test]
fn install_help_shows_flags() {
    forge()
        .args(["install", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--force"))
        .stdout(predicate::str::contains("--json"));
}

#[test]
fn install_nonexistent_path_fails() {
    forge()
        .args(["install", "/nonexistent/path"])
        .assert()
        .failure()
        .stderr(predicate::str::is_empty().not());
}

#[test]
fn validate_help_succeeds() {
    forge().args(["validate", "--help"]).assert().success();
}

#[test]
fn assemble_help_succeeds() {
    forge().args(["assemble", "--help"]).assert().success();
}

#[test]
fn copy_help_succeeds() {
    forge().args(["copy", "--help"]).assert().success();
}

#[test]
fn release_help_shows_embed() {
    forge()
        .args(["release", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--embed"));
}

#[test]
fn json_flag_accepted_globally() {
    forge()
        .args(["--json", "install", "--help"])
        .assert()
        .success();
}

#[test]
fn no_args_exits_with_error() {
    forge()
        .assert()
        .failure()
        .stderr(predicate::str::is_empty().not());
}
