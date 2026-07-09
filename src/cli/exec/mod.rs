use crate::cli::dispatch;
use commands::ontology;
use commands::parse;
use serde::Deserialize;
use serde_json::{Map, Value};
use std::ffi::{OsStr, OsString};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, Stdio};

const RUNTIMES: &[Runtime] = &[
    Runtime {
        names: &["python", "py", "uv", "uv run"],
        extensions: &[".py"],
        argv_prefix: &["uv", "run"],
    },
    Runtime {
        names: &["bash", "sh", "shell"],
        extensions: &[".sh", ".bash"],
        argv_prefix: &["bash"],
    },
    Runtime {
        names: &["typescript", "ts", "deno", "deno run"],
        extensions: &[".ts"],
        argv_prefix: &["deno", "run"],
    },
    Runtime {
        names: &["javascript", "js", "mjs", "node"],
        extensions: &[".js", ".mjs"],
        argv_prefix: &["node"],
    },
];

pub fn execute_cli(skill: &str, json: bool, rest: &[OsString]) -> Result<i32, String> {
    let parsed = parse_cli_tail(json, rest)?;
    execute(
        skill,
        parsed.script.as_deref(),
        parsed.input_json.as_deref(),
        parsed.json,
        parsed.dry_run,
        &parsed.args,
    )
}

fn execute(
    skill: &str,
    script: Option<&str>,
    input_json: Option<&str>,
    json: bool,
    dry_run: bool,
    args: &[OsString],
) -> Result<i32, String> {
    let input = parse_input(input_json)?;
    let context = ExecContext {
        cwd: std::env::current_dir().map_err(|error| error.to_string())?,
        config: ontology::load().map_err(|error| error.to_string())?,
    };
    let options = ExecOptions {
        skill: skill.to_string(),
        script: script.map(str::to_string),
        input,
        json,
        dry_run,
        args: args.to_vec(),
    };

    match run(&options, &context) {
        Ok(ExecResult::DryRun(text)) => {
            println!("{text}");
            Ok(0)
        }
        Ok(ExecResult::Completed(result)) => {
            for error in &result.validation_errors {
                eprintln!("{error}");
            }
            if options.json {
                println!("{}", result.wrapper_json());
            }
            Ok(result.exit_code)
        }
        Err(error) => {
            eprintln!("error: {}", error.message);
            Ok(error.code)
        }
    }
}

#[derive(Debug, Default)]
struct ParsedCliTail {
    script: Option<String>,
    input_json: Option<String>,
    json: bool,
    dry_run: bool,
    args: Vec<OsString>,
}

fn parse_cli_tail(global_json: bool, rest: &[OsString]) -> Result<ParsedCliTail, String> {
    let mut parsed = ParsedCliTail {
        json: global_json,
        ..ParsedCliTail::default()
    };
    let mut index = 0;
    while index < rest.len() {
        let item = rest[index].to_string_lossy();
        match item.as_ref() {
            "--" => {
                parsed.args.extend(rest[index + 1..].iter().cloned());
                return Ok(parsed);
            }
            "--script" => {
                index += 1;
                let Some(value) = rest.get(index) else {
                    return Err("--script requires a value".to_string());
                };
                parsed.script = Some(value.to_string_lossy().to_string());
            }
            "--dry-run" => {
                parsed.dry_run = true;
            }
            "--json" => {
                parsed.json = true;
                if rest.get(index + 1).is_some_and(|next| {
                    next != OsStr::new("--") && !next.to_string_lossy().starts_with("--")
                }) {
                    index += 1;
                    parsed.input_json = Some(rest[index].to_string_lossy().to_string());
                }
            }
            value => {
                if parsed.json && parsed.input_json.is_none() {
                    parsed.input_json = Some(value.to_string());
                } else {
                    return Err(format!(
                        "unexpected argument '{value}'; pass script args after --"
                    ));
                }
            }
        }
        index += 1;
    }
    Ok(parsed)
}

#[derive(Debug)]
struct ExecOptions {
    skill: String,
    script: Option<String>,
    input: Value,
    json: bool,
    dry_run: bool,
    args: Vec<OsString>,
}

#[derive(Debug)]
struct ExecContext {
    cwd: PathBuf,
    config: ontology::ResolvedConfig,
}

#[derive(Debug)]
enum ExecResult {
    DryRun(String),
    Completed(CompletedExec),
}

#[derive(Debug)]
struct CompletedExec {
    exit_code: i32,
    stdout: String,
    stderr: String,
    structured: Option<Value>,
    validation_errors: Vec<String>,
}

impl CompletedExec {
    fn wrapper_json(&self) -> String {
        serde_json::json!({
            "ok": self.exit_code == 0,
            "exit_code": self.exit_code,
            "structured": self.structured,
            "stdout": self.stdout,
            "stderr": self.stderr,
        })
        .to_string()
    }
}

#[derive(Debug)]
struct ExecError {
    code: i32,
    message: String,
}

impl ExecError {
    fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

#[derive(Debug)]
struct ResolvedExec {
    skill_dir: PathBuf,
    argv: Vec<OsString>,
    output_schema: Option<PathBuf>,
}

#[derive(Debug)]
struct Runtime {
    names: &'static [&'static str],
    extensions: &'static [&'static str],
    argv_prefix: &'static [&'static str],
}

fn run(options: &ExecOptions, context: &ExecContext) -> Result<ExecResult, ExecError> {
    let root = dispatch::forge_root_from(&context.cwd).map_err(|error| ExecError::new(2, error))?;
    let resolved = resolve_exec(options, &root, &context.config)?;
    let env = build_env(&root, &resolved.skill_dir, &context.config, &options.input);
    if options.dry_run {
        return Ok(ExecResult::DryRun(format_dry_run(&resolved.argv, &env)));
    }

    let capture = options.json || resolved.output_schema.is_some();
    let mut command = ProcessCommand::new(&resolved.argv[0]);
    command.args(&resolved.argv[1..]);
    dispatch::apply_env(&mut command, &env);

    let completed = if capture {
        let input = serde_json::to_string(&options.input)
            .map_err(|error| ExecError::new(2, format!("cannot serialize input JSON: {error}")))?;
        let output = run_capture(command, &input)?;
        let mut completed = completed_from_output(&output);
        if let Some(schema) = &resolved.output_schema {
            completed.validation_errors = validate_output(schema, &completed.stdout)?;
            if !completed.validation_errors.is_empty() {
                completed.exit_code = 1;
            }
        }
        if !options.json {
            print!("{}", completed.stdout);
            eprint!("{}", completed.stderr);
        }
        completed
    } else {
        let status = command
            .status()
            .map_err(|error| ExecError::new(2, format!("cannot run child process: {error}")))?;
        CompletedExec {
            exit_code: status.code().unwrap_or(1),
            stdout: String::new(),
            stderr: String::new(),
            structured: None,
            validation_errors: Vec::new(),
        }
    };

    Ok(ExecResult::Completed(completed))
}

fn resolve_exec(
    options: &ExecOptions,
    root: &Path,
    config: &ontology::ResolvedConfig,
) -> Result<ResolvedExec, ExecError> {
    if options.skill.contains(['/', '\\']) {
        return Err(ExecError::new(
            2,
            "skill name must not contain path separators",
        ));
    }
    let skill_dir =
        resolve_skill_dir(&options.skill, root, &config.extensions).ok_or_else(|| {
            ExecError::new(
                2,
                format!(
                    "skill '{}' not found under FORGE_ROOT or extensions",
                    options.skill
                ),
            )
        })?;
    let spec = if options.script.is_none() {
        read_exec_spec(&skill_dir)?
    } else {
        None
    };
    let script_name = options
        .script
        .as_deref()
        .or_else(|| spec.as_ref().map(|exec| exec.script.as_str()))
        .ok_or_else(|| {
            ExecError::new(3, "declare an `exec:` block in SKILL.md or pass --script")
        })?;
    let script_path = skill_dir.join(script_name);
    if !script_path.is_file() {
        return Err(ExecError::new(
            3,
            format!("script not found: {}", script_path.display()),
        ));
    }
    ensure_within(&skill_dir, &script_path)?;
    let runtime = runtime_for(
        &script_path,
        spec.as_ref().and_then(|exec| exec.runtime.as_deref()),
    )
    .ok_or_else(|| {
        ExecError::new(
            2,
            format!(
                "cannot infer runtime for {}; pass a supported runtime hint",
                script_path.display()
            ),
        )
    })?;
    let mut argv = runtime
        .argv_prefix
        .iter()
        .map(OsString::from)
        .collect::<Vec<_>>();
    argv.push(script_path.into_os_string());
    argv.extend(options.args.iter().cloned());
    let output_schema = spec
        .and_then(|exec| exec.output_schema)
        .map(|schema| skill_dir.join(schema));
    Ok(ResolvedExec {
        skill_dir,
        argv,
        output_schema,
    })
}

/// Reject a script that escapes its skill directory. Both paths are resolved to
/// their canonical form first so `..` components and symlinks cannot slip past
/// the containment check (path-boundary validation).
fn ensure_within(skill_dir: &Path, script_path: &Path) -> Result<(), ExecError> {
    let base = std::fs::canonicalize(skill_dir)
        .map_err(|error| ExecError::new(3, format!("cannot resolve skill directory: {error}")))?;
    let target = std::fs::canonicalize(script_path)
        .map_err(|error| ExecError::new(3, format!("cannot resolve script path: {error}")))?;
    if target.starts_with(&base) {
        Ok(())
    } else {
        Err(ExecError::new(
            3,
            format!("script escapes skill directory: {}", script_path.display()),
        ))
    }
}

fn resolve_skill_dir(skill: &str, root: &Path, extensions: &[PathBuf]) -> Option<PathBuf> {
    let local = root
        .join(commands::provider::ContentKind::Skills.as_str())
        .join(skill);
    if is_skill_dir(&local) {
        return Some(local);
    }
    extensions
        .iter()
        .map(|extension| {
            extension
                .join(commands::provider::ContentKind::Skills.as_str())
                .join(skill)
        })
        .find(|candidate| is_skill_dir(candidate))
}

fn is_skill_dir(path: &Path) -> bool {
    path.is_dir() && path.join("SKILL.md").is_file()
}

fn read_exec_spec(skill_dir: &Path) -> Result<Option<ExecSpec>, ExecError> {
    let skill_file = skill_dir.join("SKILL.md");
    let content = std::fs::read_to_string(&skill_file).map_err(|error| {
        ExecError::new(2, format!("cannot read {}: {error}", skill_file.display()))
    })?;
    let Some((frontmatter, _)) = parse::split_frontmatter(&content) else {
        return Ok(None);
    };
    if frontmatter.trim().is_empty() {
        return Ok(None);
    }
    let metadata: SkillFrontmatter = serde_yaml::from_str(frontmatter).map_err(|error| {
        ExecError::new(
            2,
            format!("{} has invalid frontmatter: {error}", skill_file.display()),
        )
    })?;
    Ok(metadata.exec)
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct SkillFrontmatter {
    exec: Option<ExecSpec>,
}

#[derive(Debug, Deserialize)]
struct ExecSpec {
    script: String,
    runtime: Option<String>,
    #[allow(dead_code)]
    #[serde(rename = "inputSchema")]
    input_schema: Option<String>,
    #[serde(rename = "outputSchema")]
    output_schema: Option<String>,
}

fn runtime_for(script_path: &Path, hint: Option<&str>) -> Option<&'static Runtime> {
    if let Some(hint) = hint {
        let hint = hint.to_ascii_lowercase();
        return RUNTIMES
            .iter()
            .find(|runtime| runtime.names.iter().any(|name| name == &hint.as_str()));
    }
    let extension = script_path
        .extension()
        .and_then(OsStr::to_str)
        .map(|extension| format!(".{extension}"))?;
    RUNTIMES
        .iter()
        .find(|runtime| runtime.extensions.contains(&extension.as_str()))
}

fn build_env(
    root: &Path,
    skill_dir: &Path,
    config: &ontology::ResolvedConfig,
    input: &Value,
) -> Vec<(OsString, OsString)> {
    let mut env = dispatch::forge_env(root, config);
    env.push((
        OsString::from("FORGE_SKILL_DIR"),
        skill_dir.as_os_str().to_os_string(),
    ));
    if let Value::Object(object) = input {
        env.extend(object.iter().map(|(key, value)| {
            (
                OsString::from(format!("INPUT_{}", env_key(key))),
                OsString::from(input_value(value)),
            )
        }));
    }
    env
}

fn env_key(key: &str) -> String {
    key.chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect()
}

fn input_value(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        other => other.to_string(),
    }
}

fn parse_input(input_json: Option<&str>) -> Result<Value, String> {
    let Some(input_json) = input_json else {
        return Ok(Value::Object(Map::new()));
    };
    let value: Value = serde_json::from_str(input_json)
        .map_err(|error| format!("invalid --json object: {error}"))?;
    if value.is_object() {
        Ok(value)
    } else {
        Err("--json must be a JSON object".to_string())
    }
}

fn run_capture(
    mut command: ProcessCommand,
    input: &str,
) -> Result<std::process::Output, ExecError> {
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| ExecError::new(2, format!("cannot run child process: {error}")))?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| ExecError::new(2, "cannot open child stdin"))?;
    stdin
        .write_all(input.as_bytes())
        .map_err(|error| ExecError::new(2, format!("cannot write child stdin: {error}")))?;
    drop(stdin);
    child
        .wait_with_output()
        .map_err(|error| ExecError::new(2, format!("cannot read child output: {error}")))
}

fn completed_from_output(output: &std::process::Output) -> CompletedExec {
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let structured = serde_json::from_str(&stdout).ok();
    CompletedExec {
        exit_code: output.status.code().unwrap_or(1),
        stdout,
        stderr,
        structured,
        validation_errors: Vec::new(),
    }
}

fn validate_output(schema_path: &Path, stdout: &str) -> Result<Vec<String>, ExecError> {
    let schema_content = std::fs::read_to_string(schema_path).map_err(|error| {
        ExecError::new(1, format!("cannot read {}: {error}", schema_path.display()))
    })?;
    let schema: Value = serde_json::from_str(&schema_content).map_err(|error| {
        ExecError::new(
            1,
            format!("{} is invalid JSON: {error}", schema_path.display()),
        )
    })?;
    let output: Value = serde_json::from_str(stdout)
        .map_err(|error| ExecError::new(1, format!("child stdout is not JSON: {error}")))?;
    let validator = jsonschema::options()
        .build(&schema)
        .map_err(|error| ExecError::new(1, format!("cannot compile output schema: {error}")))?;
    Ok(validator
        .iter_errors(&output)
        .map(|error| format!("outputSchema: {}: {}", error.instance_path(), error))
        .collect())
}

fn format_dry_run(argv: &[OsString], env: &[(OsString, OsString)]) -> String {
    let argv = argv
        .iter()
        .map(|part| part.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" ");
    let mut lines = vec![format!("argv: {argv}"), "env:".to_string()];
    lines.extend(env.iter().filter_map(|(key, value)| {
        let key = key.to_string_lossy();
        if key == "CI" || key.starts_with("FORGE_") || key.starts_with("INPUT_") {
            Some(format!("  {key}={}", value.to_string_lossy()))
        } else {
            None
        }
    }));
    lines.join("\n")
}

#[cfg(test)]
mod tests;
