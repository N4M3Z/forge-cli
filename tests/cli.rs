use assert_cmd::Command;
use predicates::prelude::*;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

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

#[test]
fn exec_shell_fixture_round_trips_json() {
    let root = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("module.yaml"), "name: test\n").unwrap();
    let skill = root.path().join("skills/demo");
    std::fs::create_dir_all(&skill).unwrap();
    std::fs::write(
        skill.join("SKILL.md"),
        "---\nexec:\n    script: run.sh\n---\n# Demo\n",
    )
    .unwrap();
    std::fs::write(
        skill.join("run.sh"),
        "read payload\nif [ \"$payload\" != '{\"name\":\"Ada\"}' ]; then exit 8; fi\nprintf '{\"input\":\"%s\",\"arg\":\"%s\"}\\n' \"$INPUT_NAME\" \"$1\"\n",
    )
    .unwrap();

    forge()
        .current_dir(root.path())
        .env("HOME", home.path())
        .args(["exec", "demo", "--json", "{\"name\":\"Ada\"}", "--", "x"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"ok\":true"))
        .stdout(predicate::str::contains("\"input\":\"Ada\""));
}

#[cfg(unix)]
#[test]
fn external_command_from_extension_receives_args_and_exit_code() {
    let cwd = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let extension = tempfile::tempdir().unwrap();
    let config_dir = home.path().join(".config/forge");
    std::fs::create_dir_all(&config_dir).unwrap();
    std::fs::write(
        config_dir.join("config.yaml"),
        format!("extensions:\n    - {}\n", extension.path().display()),
    )
    .unwrap();
    let script = extension.path().join("forge-hello");
    std::fs::write(
        &script,
        "#!/usr/bin/env bash\nif [ -z \"$FORGE_ROOT\" ]; then exit 7; fi\nprintf 'hello %s %s\\n' \"$1\" \"$2\"\nexit 5\n",
    )
    .unwrap();
    let mut permissions = std::fs::metadata(&script).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&script, permissions).unwrap();

    forge()
        .current_dir(cwd.path())
        .env("HOME", home.path())
        .args(["hello", "--name", "x"])
        .assert()
        .code(5)
        .stdout(predicate::str::contains("hello --name x"));
}

#[test]
fn unknown_external_command_exits_two_cleanly() {
    let cwd = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();

    forge()
        .current_dir(cwd.path())
        .env("HOME", home.path())
        .args(["does-not-exist"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains(
            "error: unknown command 'forge does-not-exist'",
        ));
}
