use super::*;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

fn context(root: &Path, launch: Launch, extensions: Vec<PathBuf>) -> LaunchContext {
    LaunchContext {
        cwd: root.to_path_buf(),
        root: root.to_path_buf(),
        config: ontology::ResolvedConfig {
            ontology: ontology::ResolvedOntology::default(),
            extensions,
            launch,
        },
    }
}

fn names(values: &[(&str, &str)]) -> Vec<(OsString, OsString)> {
    values
        .iter()
        .map(|(key, value)| (OsString::from(key), OsString::from(value)))
        .collect()
}

#[cfg(unix)]
fn write_executable(path: &Path, content: &str) {
    std::fs::write(path, content).expect("write script");
    let mut permissions = std::fs::metadata(path).expect("metadata").permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions).expect("permissions");
}

#[test]
fn shell_join_preserves_empty_args_and_quotes_values() {
    let argv = vec![
        OsString::from("a"),
        OsString::from(""),
        OsString::from("b"),
        OsString::from("two words"),
    ];

    assert_eq!(shell_join(&argv), "a '' b 'two words'");
}

#[test]
fn plan_composes_otel_then_pxpipe() {
    let dir = tempfile::tempdir().expect("tempdir");
    let launch = Launch::default();
    let middleware = vec!["otel".to_string(), "pxpipe".to_string()];

    let plan =
        compose_plan(&middleware, None, &context(dir.path(), launch, Vec::new())).expect("compose");

    assert_eq!(
        plan.env,
        names(&[
            ("OTEL_EXPORTER_OTLP_ENDPOINT", "http://127.0.0.1:4318"),
            ("OTEL_SERVICE_NAME", "forge-launch"),
        ])
    );
    assert_eq!(plan.base_url.as_deref(), Some("http://127.0.0.1:47821"));
    assert_eq!(plan.pre.len(), 1);
    assert_eq!(plan.pre[0].name, "pxpipe");
    assert_eq!(plan.pre[0].command, vec!["pxpipe"]);
}

#[test]
fn base_url_conflict_warns_and_last_wins() {
    let dir = tempfile::tempdir().expect("tempdir");
    let middleware = vec!["presidio".to_string(), "pxpipe".to_string()];

    let plan = compose_plan(
        &middleware,
        None,
        &context(dir.path(), Launch::default(), Vec::new()),
    )
    .expect("compose");

    assert_eq!(plan.base_url.as_deref(), Some("http://127.0.0.1:47821"));
    assert_eq!(plan.warnings.len(), 1);
    assert!(plan.warnings[0].contains("overrides base_url"));
    assert!(plan.warnings[0].contains("pxpipe"));
}

#[test]
fn tmux_flag_adds_outer_wrap() {
    let dir = tempfile::tempdir().expect("tempdir");
    let plan = compose_plan(
        &["otel".to_string()],
        Some("work"),
        &context(dir.path(), Launch::default(), Vec::new()),
    )
    .expect("compose");
    let tool = resolve_tool("claude", &Launch::default());
    let argv = build_argv(
        &tool,
        &[OsString::from("--model"), OsString::from("opus")],
        &plan,
    );

    assert_eq!(plan.wrap.len(), 1);
    assert_eq!(
        plan.wrap[0],
        vec![
            OsString::from("tmux"),
            OsString::from("new-session"),
            OsString::from("-A"),
            OsString::from("-s"),
            OsString::from("work"),
        ]
    );
    assert_eq!(argv[0], OsString::from("tmux"));
    assert!(
        argv.last()
            .expect("tmux shell command")
            .to_string_lossy()
            .contains("claude --model opus")
    );
}

#[test]
fn docker_wrap_forwards_final_env_to_container() {
    let dir = tempfile::tempdir().expect("tempdir");
    let middleware = vec!["docker".to_string(), "pxpipe".to_string()];
    let plan = compose_plan(
        &middleware,
        None,
        &context(dir.path(), Launch::default(), Vec::new()),
    )
    .expect("compose");
    let tool = resolve_tool("claude", &Launch::default());
    let argv = build_argv(&tool, &[], &plan);

    let rendered = display_argv(&argv);
    assert!(rendered.contains("-e ANTHROPIC_BASE_URL=http://127.0.0.1:47821"));
    assert!(rendered.contains("ghcr.io/n4m3z/forge-coding-tool:latest claude"));
}

#[test]
fn tmux_wrap_forwards_final_env_to_inner_command() {
    let dir = tempfile::tempdir().expect("tempdir");
    let plan = compose_plan(
        &["pxpipe".to_string()],
        Some("work"),
        &context(dir.path(), Launch::default(), Vec::new()),
    )
    .expect("compose");
    let tool = resolve_tool("claude", &Launch::default());
    let argv = build_argv(&tool, &[OsString::from("--version")], &plan);
    let inner = argv.last().expect("tmux shell command").to_string_lossy();

    assert!(inner.contains("env ANTHROPIC_BASE_URL=http://127.0.0.1:47821 claude --version"));
}

#[test]
fn unwrapped_launch_keeps_env_out_of_argv() {
    let mut plan = LaunchPlan {
        base_url: Some("http://127.0.0.1:47821".to_string()),
        ..LaunchPlan::default()
    };
    plan.env.push((
        OsString::from("OTEL_SERVICE_NAME"),
        OsString::from("forge-launch"),
    ));
    let tool = resolve_tool("claude", &Launch::default());
    let argv = build_argv(&tool, &[OsString::from("--version")], &plan);

    assert_eq!(
        argv,
        vec![OsString::from("claude"), OsString::from("--version")]
    );
    assert_eq!(
        process_env(&tool, &plan),
        names(&[
            ("OTEL_SERVICE_NAME", "forge-launch"),
            ("ANTHROPIC_BASE_URL", "http://127.0.0.1:47821"),
        ])
    );
}

#[test]
fn wrapped_launch_does_not_apply_env_to_wrapper_process() {
    let dir = tempfile::tempdir().expect("tempdir");
    let plan = compose_plan(
        &["docker".to_string(), "pxpipe".to_string()],
        None,
        &context(dir.path(), Launch::default(), Vec::new()),
    )
    .expect("compose");
    let tool = resolve_tool("claude", &Launch::default());

    assert!(process_env(&tool, &plan).is_empty());
}

#[test]
fn dry_run_prints_plan_without_launching() {
    let dir = tempfile::tempdir().expect("tempdir");
    let middleware = vec!["otel".to_string(), "pxpipe".to_string()];
    let plan = compose_plan(
        &middleware,
        Some("x"),
        &context(dir.path(), Launch::default(), Vec::new()),
    )
    .expect("compose");
    let tool = resolve_tool("claude", &Launch::default());
    let argv = build_argv(
        &tool,
        &[OsString::from("--model"), OsString::from("opus")],
        &plan,
    );
    let output = format_dry_run(&tool, &argv, &plan);

    assert!(output.contains("tool: claude"));
    assert!(output.contains("OTEL_EXPORTER_OTLP_ENDPOINT=http://127.0.0.1:4318"));
    assert!(output.contains("ANTHROPIC_BASE_URL=http://127.0.0.1:47821"));
    assert!(output.contains("base_url: http://127.0.0.1:47821"));
    assert!(output.contains("tmux new-session -A -s x"));
    assert!(output.contains("pxpipe 127.0.0.1:47821"));
}

#[test]
fn merge_patch_rejects_sensitive_env_and_keeps_normal_env() {
    let mut plan = LaunchPlan::default();
    merge_patch(
        &mut plan,
        "external",
        PlanPatch {
            env: vec![
                ("LD_PRELOAD".to_string(), "/tmp/hijack.dylib".to_string()),
                (
                    "OTEL_RESOURCE_ATTRIBUTES".to_string(),
                    "service.name=forge".to_string(),
                ),
            ],
            ..PlanPatch::default()
        },
    );

    assert_eq!(
        plan.env,
        names(&[("OTEL_RESOURCE_ATTRIBUTES", "service.name=forge")])
    );
    assert_eq!(plan.warnings.len(), 1);
    assert!(plan.warnings[0].contains("sensitive env LD_PRELOAD"));
    assert!(plan.warnings[0].contains("dropping"));
}

#[test]
fn external_base_url_from_none_warns() {
    let mut plan = LaunchPlan::default();
    merge_patch(
        &mut plan,
        "external",
        PlanPatch {
            base_url: Some("https://api.example.invalid".to_string()),
            ..PlanPatch::default()
        },
    );

    assert_eq!(
        plan.base_url.as_deref(),
        Some("https://api.example.invalid")
    );
    assert_eq!(plan.warnings.len(), 1);
    assert!(plan.warnings[0].contains("external middleware 'external'"));
    assert!(plan.warnings[0].contains("https://api.example.invalid"));
}

#[test]
fn unknown_middleware_without_script_errors_with_known_list() {
    let dir = tempfile::tempdir().expect("tempdir");
    let error = compose_plan(
        &["missing".to_string()],
        None,
        &context(dir.path(), Launch::default(), Vec::new()),
    )
    .expect_err("unknown middleware");

    assert!(error.contains("unknown middleware 'missing'"));
    assert!(error.contains("pxpipe, otel, presidio, squid, docker, tmux"));
}

#[cfg(unix)]
#[test]
fn external_middleware_patch_is_merged_from_extension() {
    let dir = tempfile::tempdir().expect("tempdir");
    let extension = tempfile::tempdir().expect("extension");
    let script = extension.path().join("forge-launch-mw-test");
    write_executable(
        &script,
        "#!/usr/bin/env bash\ncat >/dev/null\nprintf '%s\\n' '{\"env\":[[\"TEST_MW\",\"enabled\"]]}'\n",
    );

    let plan = compose_plan(
        &["test".to_string()],
        None,
        &context(
            dir.path(),
            Launch::default(),
            vec![extension.path().to_path_buf()],
        ),
    )
    .expect("compose external");

    assert_eq!(plan.env, names(&[("TEST_MW", "enabled")]));
}

#[cfg(unix)]
#[test]
fn external_middleware_oversized_stdout_errors() {
    let dir = tempfile::tempdir().expect("tempdir");
    let script = dir.path().join("forge-launch-mw-large");
    write_executable(
        &script,
        "#!/usr/bin/env bash\ncat >/dev/null\nprintf '%1048577s' ''\n",
    );

    let error =
        run_external_middleware(&script, &LaunchPlan::default()).expect_err("oversized stdout");

    assert!(error.contains("stdout exceeded 1048576 byte limit"));
}

#[test]
fn pxpipe_pre_step_command_with_args_builds_argv() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut launch = Launch::default();
    launch.middleware.pxpipe.command = "pxpipe --port 47821".to_string();
    launch.middleware.pxpipe.log_path = dir.path().join("pxpipe.log").to_string_lossy().to_string();
    let plan = compose_plan(
        &["pxpipe".to_string()],
        None,
        &context(dir.path(), launch, Vec::new()),
    )
    .expect("compose");

    assert_eq!(plan.pre[0].command, vec!["pxpipe", "--port", "47821"]);
    let command = build_pre_step_command(&plan.pre[0])
        .expect("pre-step command")
        .expect("configured command");
    let args = command.get_args().map(OsString::from).collect::<Vec<_>>();

    assert_eq!(command.get_program(), OsStr::new("pxpipe"));
    assert_eq!(
        args,
        vec![OsString::from("--port"), OsString::from("47821")]
    );
}
