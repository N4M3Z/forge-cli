use crate::cli::dispatch;
use commands::ontology::{self, DockerConfig, Launch};
use serde::{Deserialize, Serialize};
use std::ffi::{OsStr, OsString};
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, Stdio};
use std::time::Duration;

const BUILT_INS: &[&str] = &["pxpipe", "otel", "presidio", "squid", "docker", "tmux"];
const MIDDLEWARE_STDOUT_LIMIT_BYTES: usize = 1024 * 1024;
const MIDDLEWARE_STDOUT_READ_LIMIT_BYTES: u64 = 1024 * 1024 + 1;
const SENSITIVE_ENV_KEYS: &[&str] = &[
    "PATH",
    "LD_PRELOAD",
    "LD_LIBRARY_PATH",
    "DYLD_INSERT_LIBRARIES",
    "DYLD_LIBRARY_PATH",
];

pub fn execute_cli(tool: &str, rest: &[OsString]) -> Result<i32, String> {
    let cwd = std::env::current_dir().map_err(|error| error.to_string())?;
    let root = dispatch::forge_root_from(&cwd)?;
    let config = ontology::load().map_err(|error| error.to_string())?;
    let options = parse_cli_tail(rest, &config.launch)?;
    let context = LaunchContext { cwd, root, config };
    let tool = resolve_tool(tool, &context.config.launch);
    let mut plan = compose_plan(&options.middleware, options.tmux.as_deref(), &context)?;
    let argv = build_argv(&tool, &options.args, &plan);
    if options.dry_run {
        println!("{}", format_dry_run(&tool, &argv, &plan));
        return Ok(0);
    }
    for warning in &plan.warnings {
        eprintln!("warning: {warning}");
    }
    run_pre_steps(&mut plan);
    run_child(&argv, &process_env(&tool, &plan))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedLaunchTail {
    middleware: Vec<String>,
    tmux: Option<String>,
    dry_run: bool,
    args: Vec<OsString>,
}

#[derive(Debug)]
struct LaunchContext {
    cwd: PathBuf,
    root: PathBuf,
    config: ontology::ResolvedConfig,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResolvedTool {
    name: String,
    binary: OsString,
    base_url_env: Option<OsString>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct LaunchPlan {
    env: Vec<(OsString, OsString)>,
    base_url: Option<String>,
    wrap: Vec<Vec<OsString>>,
    pre: Vec<PreStep>,
    warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct PreStep {
    name: String,
    host: String,
    port: u16,
    command: Vec<String>,
    log_path: Option<String>,
    optional: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct PlanPatch {
    env: Vec<(String, String)>,
    base_url: Option<String>,
    wrap: Vec<Vec<String>>,
    pre: Vec<PreStep>,
}

#[derive(Debug, Serialize)]
struct SerializablePlan {
    env: Vec<(String, String)>,
    base_url: Option<String>,
    wrap: Vec<Vec<String>>,
    pre: Vec<PreStep>,
}

fn parse_cli_tail(rest: &[OsString], launch: &Launch) -> Result<ParsedLaunchTail, String> {
    let mut parsed = ParsedLaunchTail {
        middleware: launch.default_with.clone(),
        tmux: None,
        dry_run: false,
        args: Vec::new(),
    };
    let mut saw_explicit_chain = false;
    let mut index = 0;
    while index < rest.len() {
        let item = rest[index].to_string_lossy();
        match item.as_ref() {
            "--" => {
                parsed.args.extend(rest[index + 1..].iter().cloned());
                return Ok(parsed);
            }
            "--dry-run" => parsed.dry_run = true,
            "--pxpipe" => {
                clear_default_chain(&mut parsed.middleware, &mut saw_explicit_chain);
                parsed.middleware.push("pxpipe".to_string());
            }
            "--direct" => {
                parsed.middleware.clear();
                saw_explicit_chain = true;
            }
            "--with" => {
                index += 1;
                let Some(value) = rest.get(index) else {
                    return Err("--with requires a comma-separated value".to_string());
                };
                clear_default_chain(&mut parsed.middleware, &mut saw_explicit_chain);
                parsed.middleware.extend(parse_middleware_list(value));
            }
            "--tmux" => {
                parsed.tmux = Some(String::new());
            }
            value if value.starts_with("--with=") => {
                clear_default_chain(&mut parsed.middleware, &mut saw_explicit_chain);
                parsed
                    .middleware
                    .extend(split_middleware_list(value.trim_start_matches("--with=")));
            }
            value if value.starts_with("--tmux=") => {
                parsed.tmux = Some(value.trim_start_matches("--tmux=").to_string());
            }
            _ => {
                parsed.args.extend(rest[index..].iter().cloned());
                return Ok(parsed);
            }
        }
        index += 1;
    }
    Ok(parsed)
}

fn clear_default_chain(chain: &mut Vec<String>, saw_explicit_chain: &mut bool) {
    if !*saw_explicit_chain {
        chain.clear();
        *saw_explicit_chain = true;
    }
}

fn parse_middleware_list(value: &OsString) -> Vec<String> {
    split_middleware_list(&value.to_string_lossy())
}

fn split_middleware_list(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn resolve_tool(name: &str, launch: &Launch) -> ResolvedTool {
    let configured = launch.tools.get(name);
    let default_base_env = (name == "claude").then(|| "ANTHROPIC_BASE_URL".to_string());
    let binary = configured
        .and_then(|tool| tool.binary.as_ref())
        .map_or_else(|| name.to_string(), Clone::clone);
    let base_url_env = configured
        .and_then(|tool| tool.base_url_env.as_ref())
        .cloned()
        .or(default_base_env)
        .map(OsString::from);
    ResolvedTool {
        name: name.to_string(),
        binary: OsString::from(binary),
        base_url_env,
    }
}

fn compose_plan(
    middleware: &[String],
    tmux_name: Option<&str>,
    context: &LaunchContext,
) -> Result<LaunchPlan, String> {
    let mut plan = LaunchPlan::default();
    for name in middleware {
        apply_middleware(&mut plan, name, context)?;
    }
    if let Some(name) = tmux_name {
        let session = if name.is_empty() {
            context
                .cwd
                .file_name()
                .and_then(OsStr::to_str)
                .unwrap_or("forge")
                .to_string()
        } else {
            name.to_string()
        };
        apply_builtin(&mut plan, "tmux", Some(&session), &context.config.launch)?;
    }
    Ok(plan)
}

fn apply_middleware(
    plan: &mut LaunchPlan,
    name: &str,
    context: &LaunchContext,
) -> Result<(), String> {
    if BUILT_INS.contains(&name) {
        return apply_builtin(plan, name, None, &context.config.launch);
    }
    apply_external(plan, name, context)
}

fn apply_builtin(
    plan: &mut LaunchPlan,
    name: &str,
    argument: Option<&str>,
    launch: &Launch,
) -> Result<(), String> {
    match name {
        "pxpipe" => {
            let config = &launch.middleware.pxpipe;
            set_base_url(plan, name, config.base_url.clone());
            plan.pre.push(PreStep {
                name: "pxpipe".to_string(),
                host: config.host.clone(),
                port: config.port,
                command: split_pre_step_command(&config.command),
                log_path: Some(config.log_path.clone()),
                optional: true,
            });
        }
        "otel" => {
            let config = &launch.middleware.otel;
            set_env(
                plan,
                "otel",
                "OTEL_EXPORTER_OTLP_ENDPOINT",
                &config.endpoint,
            );
            set_env(plan, "otel", "OTEL_SERVICE_NAME", &config.service_name);
        }
        "presidio" => {
            let config = &launch.middleware.presidio;
            set_base_url(plan, name, config.base_url.clone());
            plan.pre.push(PreStep {
                name: "presidio".to_string(),
                host: config.host.clone(),
                port: config.port,
                command: Vec::new(),
                log_path: None,
                optional: true,
            });
        }
        "squid" => {
            let config = &launch.middleware.squid;
            set_env(plan, "squid", "HTTP_PROXY", &config.http_proxy);
            set_env(plan, "squid", "HTTPS_PROXY", &config.https_proxy);
        }
        "docker" => plan.wrap.push(docker_wrap(&launch.middleware.docker)),
        "tmux" => {
            let session = argument.unwrap_or("forge");
            plan.wrap.push(vec![
                OsString::from("tmux"),
                OsString::from("new-session"),
                OsString::from("-A"),
                OsString::from("-s"),
                OsString::from(session),
            ]);
        }
        _ => {
            return Err(format!(
                "unknown middleware '{name}'; known middleware: {}",
                BUILT_INS.join(", ")
            ));
        }
    }
    Ok(())
}

fn docker_wrap(config: &DockerConfig) -> Vec<OsString> {
    let mut wrap = vec![OsString::from("docker"), OsString::from("run")];
    wrap.extend(config.args.iter().map(OsString::from));
    wrap.push(OsString::from(&config.image));
    wrap
}

fn split_pre_step_command(command: &str) -> Vec<String> {
    command
        .split_whitespace()
        .map(ToString::to_string)
        .collect()
}

fn set_env(plan: &mut LaunchPlan, middleware: &str, key: &str, value: &str) {
    let key_os = OsString::from(key);
    let value_os = OsString::from(value);
    if let Some((_, existing)) = plan
        .env
        .iter_mut()
        .find(|(candidate, _)| candidate == &key_os)
    {
        if existing != &value_os {
            plan.warnings.push(format!(
                "middleware '{middleware}' overrides env {key} from '{}' to '{value}'",
                existing.to_string_lossy()
            ));
        }
        *existing = value_os;
        return;
    }
    plan.env.push((key_os, value_os));
}

fn set_base_url(plan: &mut LaunchPlan, middleware: &str, value: String) {
    if let Some(existing) = &plan.base_url
        && existing != &value
    {
        plan.warnings.push(format!(
            "middleware '{middleware}' overrides base_url from '{existing}' to '{value}'"
        ));
    }
    plan.base_url = Some(value);
}

fn set_external_base_url(plan: &mut LaunchPlan, middleware: &str, value: String) {
    if let Some(existing) = &plan.base_url
        && existing != &value
    {
        plan.warnings.push(format!(
            "external middleware '{middleware}' sets base_url to '{value}', overriding '{existing}'"
        ));
    } else {
        plan.warnings.push(format!(
            "external middleware '{middleware}' sets base_url to '{value}'; verify this API endpoint is trusted"
        ));
    }
    plan.base_url = Some(value);
}

fn apply_external(
    plan: &mut LaunchPlan,
    name: &str,
    context: &LaunchContext,
) -> Result<(), String> {
    if name.contains(['/', '\\']) {
        return Err("middleware name must not contain path separators".to_string());
    }
    let command_name = format!("forge-launch-mw-{name}");
    let Some(executable) =
        dispatch::resolve_external(&command_name, &context.root, &context.config.extensions)
    else {
        return Err(format!(
            "unknown middleware '{name}'; known middleware: {}",
            BUILT_INS.join(", ")
        ));
    };
    let patch = run_external_middleware(&executable, plan)?;
    merge_patch(plan, name, patch);
    Ok(())
}

fn run_external_middleware(executable: &Path, plan: &LaunchPlan) -> Result<PlanPatch, String> {
    let input = serde_json::to_string(&serializable_plan(plan))
        .map_err(|error| format!("cannot serialize launch plan: {error}"))?;
    let mut child = ProcessCommand::new(executable)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("cannot run {}: {error}", executable.display()))?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| "cannot open middleware stdin".to_string())?;
    stdin
        .write_all(input.as_bytes())
        .map_err(|error| format!("cannot write middleware stdin: {error}"))?;
    drop(stdin);
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "cannot open middleware stdout".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "cannot open middleware stderr".to_string())?;
    let executable_name = executable.display().to_string();
    let stdout_handle = std::thread::spawn(move || read_limited_stdout(stdout, &executable_name));
    let stderr_handle = std::thread::spawn(move || read_stderr(stderr));
    let stdout = stdout_handle
        .join()
        .map_err(|_| "middleware stdout reader panicked".to_string())?;
    let stdout = match stdout {
        Ok(output) => output,
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            let _ = stderr_handle.join();
            return Err(error);
        }
    };
    let status = child
        .wait()
        .map_err(|error| format!("cannot read middleware output: {error}"))?;
    let stderr = stderr_handle
        .join()
        .map_err(|_| "middleware stderr reader panicked".to_string())?
        .map_err(|error| format!("cannot read middleware stderr: {error}"))?;
    if !status.success() {
        return Err(format!(
            "{} exited with {}; {}",
            executable.display(),
            status.code().unwrap_or(1),
            stderr.trim()
        ));
    }
    serde_json::from_str(&stdout)
        .map_err(|error| format!("{} emitted invalid JSON: {error}", executable.display()))
}

fn read_limited_stdout(mut stdout: impl Read, executable: &str) -> Result<String, String> {
    let mut output = String::new();
    let bytes_read = stdout
        .by_ref()
        .take(MIDDLEWARE_STDOUT_READ_LIMIT_BYTES)
        .read_to_string(&mut output)
        .map_err(|error| format!("cannot read {executable} stdout: {error}"))?;
    if bytes_read > MIDDLEWARE_STDOUT_LIMIT_BYTES {
        return Err(format!(
            "{executable} stdout exceeded {MIDDLEWARE_STDOUT_LIMIT_BYTES} byte limit"
        ));
    }
    Ok(output)
}

fn read_stderr(mut stderr: impl Read) -> Result<String, std::io::Error> {
    let mut output = Vec::new();
    stderr.read_to_end(&mut output)?;
    Ok(String::from_utf8_lossy(&output).into_owned())
}

fn merge_patch(plan: &mut LaunchPlan, middleware: &str, patch: PlanPatch) {
    for (key, value) in patch.env {
        if is_sensitive_env_key(&key) {
            plan.warnings.push(format!(
                "middleware '{middleware}' attempted to set sensitive env {key}; dropping it"
            ));
        } else {
            set_env(plan, middleware, &key, &value);
        }
    }
    if let Some(base_url) = patch.base_url {
        set_external_base_url(plan, middleware, base_url);
    }
    plan.wrap.extend(patch.wrap.into_iter().map(|wrap| {
        wrap.into_iter()
            .map(OsString::from)
            .collect::<Vec<OsString>>()
    }));
    plan.pre.extend(patch.pre);
}

fn is_sensitive_env_key(key: &str) -> bool {
    SENSITIVE_ENV_KEYS
        .iter()
        .any(|sensitive| sensitive.eq_ignore_ascii_case(key))
}

fn build_argv(tool: &ResolvedTool, child_args: &[OsString], plan: &LaunchPlan) -> Vec<OsString> {
    let mut command_line = vec![tool.binary.clone()];
    command_line.extend(child_args.iter().cloned());
    let env = final_env(tool, plan);
    for wrap in &plan.wrap {
        command_line = apply_wrap(wrap, &command_line, &env);
    }
    command_line
}

fn apply_wrap(
    wrap: &[OsString],
    inner: &[OsString],
    env: &[(OsString, OsString)],
) -> Vec<OsString> {
    let mut argv = wrap.to_vec();
    if wrap.first().is_some_and(|command| command == "tmux") {
        argv.push(OsString::from(shell_join(&env_command(inner, env))));
    } else if wrap.first().is_some_and(|command| command == "docker") {
        argv = docker_argv(wrap, inner, env);
    } else {
        argv.extend(inner.iter().cloned());
    }
    argv
}

fn is_explicit_env_wrapper(wrap: &[OsString]) -> bool {
    wrap.first()
        .is_some_and(|command| command == "docker" || command == "tmux")
}

fn env_command(inner: &[OsString], env: &[(OsString, OsString)]) -> Vec<OsString> {
    if env.is_empty() {
        return inner.to_vec();
    }
    let mut argv = vec![OsString::from("env")];
    argv.extend(env.iter().map(env_assignment));
    argv.extend(inner.iter().cloned());
    argv
}

fn docker_argv(
    wrap: &[OsString],
    inner: &[OsString],
    env: &[(OsString, OsString)],
) -> Vec<OsString> {
    let Some((image, docker_prefix)) = wrap.split_last() else {
        return inner.to_vec();
    };
    let mut argv = docker_prefix.to_vec();
    for assignment in env.iter().map(env_assignment) {
        argv.push(OsString::from("-e"));
        argv.push(assignment);
    }
    argv.push(image.clone());
    argv.extend(inner.iter().cloned());
    argv
}

fn env_assignment((key, value): &(OsString, OsString)) -> OsString {
    OsString::from(format!(
        "{}={}",
        key.to_string_lossy(),
        value.to_string_lossy()
    ))
}

fn final_env(tool: &ResolvedTool, plan: &LaunchPlan) -> Vec<(OsString, OsString)> {
    let mut env = plan.env.clone();
    if let (Some(base_url), Some(env_key)) = (&plan.base_url, &tool.base_url_env) {
        env.push((env_key.clone(), OsString::from(base_url)));
    }
    env
}

fn process_env(tool: &ResolvedTool, plan: &LaunchPlan) -> Vec<(OsString, OsString)> {
    if plan.wrap.iter().any(|wrap| is_explicit_env_wrapper(wrap)) {
        return Vec::new();
    }
    final_env(tool, plan)
}

fn run_child(argv: &[OsString], env: &[(OsString, OsString)]) -> Result<i32, String> {
    let Some(command_name) = argv.first() else {
        return Err("launch plan produced an empty argv".to_string());
    };
    let mut command = ProcessCommand::new(command_name);
    command.args(&argv[1..]);
    dispatch::apply_env(&mut command, env);
    let status = command
        .status()
        .map_err(|error| format!("cannot run child process: {error}"))?;
    Ok(status.code().unwrap_or(1))
}

fn run_pre_steps(plan: &mut LaunchPlan) {
    for step in &plan.pre {
        if is_listening(&step.host, step.port) {
            continue;
        }
        if step.command.is_empty() {
            eprintln!(
                "warning: middleware '{}' is not listening on {}:{}",
                step.name, step.host, step.port
            );
            continue;
        }
        if let Err(error) = start_pre_step(step) {
            eprintln!("warning: {error}");
        }
        std::thread::sleep(Duration::from_millis(700));
        if !is_listening(&step.host, step.port) && step.optional {
            eprintln!(
                "warning: middleware '{}' did not become ready on {}:{}",
                step.name, step.host, step.port
            );
        }
    }
}

fn start_pre_step(step: &PreStep) -> Result<(), String> {
    let Some(mut process) = build_pre_step_command(step)? else {
        return Ok(());
    };
    process
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("cannot start middleware '{}': {error}", step.name))
}

fn build_pre_step_command(step: &PreStep) -> Result<Option<ProcessCommand>, String> {
    let Some(command) = step.command.first() else {
        return Ok(None);
    };
    let mut process = ProcessCommand::new(command);
    process.args(&step.command[1..]);
    if step.name == "pxpipe" {
        configure_pxpipe_stdio(&mut process, step.log_path.as_ref())?;
    } else {
        process.stdout(Stdio::null()).stderr(Stdio::null());
    }
    Ok(Some(process))
}

fn configure_pxpipe_stdio(
    process: &mut ProcessCommand,
    log_path: Option<&String>,
) -> Result<(), String> {
    let Some(log_path) = log_path else {
        process.stdout(Stdio::null()).stderr(Stdio::null());
        return Ok(());
    };
    let path = ontology::expand_tilde(log_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    }
    let stdout = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|error| format!("cannot open {}: {error}", path.display()))?;
    let stderr = stdout
        .try_clone()
        .map_err(|error| format!("cannot clone {}: {error}", path.display()))?;
    process
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr));
    Ok(())
}

fn is_listening(host: &str, port: u16) -> bool {
    let Ok(address) = format!("{host}:{port}").parse::<SocketAddr>() else {
        return false;
    };
    TcpStream::connect_timeout(&address, Duration::from_millis(150)).is_ok()
}

fn format_dry_run(tool: &ResolvedTool, argv: &[OsString], plan: &LaunchPlan) -> String {
    let mut lines = vec![
        format!("tool: {}", tool.name),
        format!("argv: {}", display_argv(argv)),
        "env:".to_string(),
    ];
    lines.extend(
        final_env(tool, plan)
            .iter()
            .map(|(key, value)| format!("  {}={}", key.to_string_lossy(), value.to_string_lossy())),
    );
    lines.push(format!(
        "base_url: {}",
        plan.base_url.as_deref().unwrap_or("<none>")
    ));
    lines.push("wrap:".to_string());
    lines.extend(
        plan.wrap
            .iter()
            .map(|wrap| format!("  {}", display_argv(wrap))),
    );
    lines.push("pre:".to_string());
    lines.extend(plan.pre.iter().map(|step| {
        let command = if step.command.is_empty() {
            "<check-only>".to_string()
        } else {
            step.command.join(" ")
        };
        format!("  {} {}:{} {}", step.name, step.host, step.port, command)
    }));
    if !plan.warnings.is_empty() {
        lines.push("warnings:".to_string());
        lines.extend(plan.warnings.iter().map(|warning| format!("  {warning}")));
    }
    lines.join("\n")
}

fn display_argv(argv: &[OsString]) -> String {
    argv.iter()
        .map(|part| part.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" ")
}

fn shell_join(argv: &[OsString]) -> String {
    argv.iter()
        .map(|part| shell_quote(&part.to_string_lossy()))
        .collect::<Vec<_>>()
        .join(" ")
}

fn shell_quote(value: &str) -> String {
    if value.is_empty() {
        return "''".to_string();
    }
    if value
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || "-_./:=@".contains(character))
    {
        return value.to_string();
    }
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn serializable_plan(plan: &LaunchPlan) -> SerializablePlan {
    SerializablePlan {
        env: plan
            .env
            .iter()
            .map(|(key, value)| {
                (
                    key.to_string_lossy().to_string(),
                    value.to_string_lossy().to_string(),
                )
            })
            .collect(),
        base_url: plan.base_url.clone(),
        wrap: plan
            .wrap
            .iter()
            .map(|wrap| {
                wrap.iter()
                    .map(|part| part.to_string_lossy().to_string())
                    .collect()
            })
            .collect(),
        pre: plan.pre.clone(),
    }
}

#[cfg(test)]
mod tests;
