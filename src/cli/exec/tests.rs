use super::*;

fn context(cwd: PathBuf) -> ExecContext {
    ExecContext {
        cwd,
        config: ontology::ResolvedConfig::default(),
    }
}

fn make_skill(root: &Path, frontmatter: &str, script: &str) -> PathBuf {
    let skill = root.join("skills/demo");
    std::fs::create_dir_all(&skill).expect("skill dir");
    std::fs::write(
        skill.join("SKILL.md"),
        format!("---\n{frontmatter}---\n# Demo\n"),
    )
    .expect("skill file");
    std::fs::write(skill.join("run.sh"), script).expect("script");
    skill
}

#[test]
fn extension_runtime_dispatch_picks_shell() {
    let runtime = runtime_for(Path::new("run.sh"), None).expect("runtime");
    assert_eq!(runtime.argv_prefix, &["bash"]);
}

#[test]
fn shell_fixture_round_trips_json_and_input_env() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();
    std::fs::write(root.join("module.yaml"), "name: test\n").expect("module");
    make_skill(
        root,
        "exec:\n    script: run.sh\n",
        "read payload\nif [ \"$payload\" != '{\"name\":\"Ada\"}' ]; then exit 8; fi\nif [ -z \"$FORGE_ROOT\" ]; then exit 9; fi\nprintf '{\"input\":\"%s\",\"arg\":\"%s\"}\\n' \"$INPUT_NAME\" \"$1\"\n",
    );
    let options = ExecOptions {
        skill: "demo".to_string(),
        script: None,
        input: serde_json::json!({ "name": "Ada" }),
        json: true,
        dry_run: false,
        args: vec![OsString::from("x")],
    };

    let ExecResult::Completed(result) = run(&options, &context(root.to_path_buf())).expect("run")
    else {
        panic!("expected completed exec");
    };
    assert_eq!(result.exit_code, 0);
    assert_eq!(
        result.structured,
        Some(serde_json::json!({ "input": "Ada", "arg": "x" }))
    );
}

#[test]
fn output_schema_bad_payload_fails_validation() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();
    std::fs::write(root.join("module.yaml"), "name: test\n").expect("module");
    let skill = make_skill(
        root,
        "exec:\n    script: run.sh\n    outputSchema: schema.json\n",
        "printf '{\"count\":\"no\"}\\n'\n",
    );
    std::fs::write(
        skill.join("schema.json"),
        r#"{"type":"object","properties":{"count":{"type":"integer"}},"required":["count"]}"#,
    )
    .expect("schema");
    let options = ExecOptions {
        skill: "demo".to_string(),
        script: None,
        input: serde_json::json!({}),
        json: true,
        dry_run: false,
        args: Vec::new(),
    };

    let ExecResult::Completed(result) = run(&options, &context(root.to_path_buf())).expect("run")
    else {
        panic!("expected completed exec");
    };
    assert_eq!(result.exit_code, 1);
    assert!(!result.validation_errors.is_empty());
}

#[test]
fn dry_run_emits_command_without_running() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();
    std::fs::write(root.join("module.yaml"), "name: test\n").expect("module");
    make_skill(
        root,
        "exec:\n    script: run.sh\n",
        "touch SHOULD_NOT_EXIST\n",
    );
    let options = ExecOptions {
        skill: "demo".to_string(),
        script: None,
        input: serde_json::json!({ "name": "Ada" }),
        json: true,
        dry_run: true,
        args: Vec::new(),
    };

    let ExecResult::DryRun(text) = run(&options, &context(root.to_path_buf())).expect("dry run")
    else {
        panic!("expected dry run");
    };
    assert!(text.contains("argv: bash"));
    assert!(text.contains("INPUT_NAME=Ada"));
    assert!(!root.join("SHOULD_NOT_EXIST").exists());
}

#[test]
fn missing_script_guidance_exits_three() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();
    std::fs::write(root.join("module.yaml"), "name: test\n").expect("module");
    make_skill(root, "", "printf '{}\\n'\n");
    let options = ExecOptions {
        skill: "demo".to_string(),
        script: None,
        input: serde_json::json!({}),
        json: true,
        dry_run: false,
        args: Vec::new(),
    };

    let error = run(&options, &context(root.to_path_buf())).expect_err("missing exec");
    assert_eq!(error.code, 3);
    assert!(error.message.contains("declare an `exec:` block"));
}

#[test]
fn script_escaping_skill_dir_is_rejected() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();
    std::fs::write(root.join("module.yaml"), "name: test\n").expect("module");
    make_skill(root, "exec:\n    script: run.sh\n", "printf '{}\\n'\n");
    std::fs::write(root.join("evil.sh"), "printf 'pwned\\n'\n").expect("evil");
    let options = ExecOptions {
        skill: "demo".to_string(),
        script: Some("../../evil.sh".to_string()),
        input: serde_json::json!({}),
        json: true,
        dry_run: false,
        args: Vec::new(),
    };

    let error = run(&options, &context(root.to_path_buf())).expect_err("traversal rejected");
    assert_eq!(error.code, 3);
    assert!(error.message.contains("escapes skill directory"));
}

#[test]
fn skill_name_with_separator_is_rejected() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();
    std::fs::write(root.join("module.yaml"), "name: test\n").expect("module");
    let options = ExecOptions {
        skill: "../../etc".to_string(),
        script: None,
        input: serde_json::json!({}),
        json: true,
        dry_run: false,
        args: Vec::new(),
    };

    let error = run(&options, &context(root.to_path_buf())).expect_err("separator rejected");
    assert_eq!(error.code, 2);
    assert!(error.message.contains("path separators"));
}
