use commands::ontology;
use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;

pub fn external(args: &[OsString]) -> Result<i32, String> {
    external_with_context(
        args,
        &env::current_dir().map_err(|error| error.to_string())?,
    )
}

fn external_with_context(args: &[OsString], cwd: &Path) -> Result<i32, String> {
    let Some(verb) = args.first() else {
        eprintln!("error: missing external command");
        return Ok(2);
    };
    let Some(verb_text) = verb.to_str() else {
        eprintln!("error: external command is not valid UTF-8");
        return Ok(2);
    };
    if verb_text.contains(['/', '\\']) {
        eprintln!(
            "error: invalid command 'forge {verb_text}': name must not contain path separators"
        );
        return Ok(2);
    }
    let root = forge_root_from(cwd)?;
    let config = ontology::load().map_err(|error| error.to_string())?;
    let command_name = format!("forge-{verb_text}");

    let Some(executable) = resolve_external(&command_name, &root, &config.extensions) else {
        eprintln!("error: unknown command 'forge {verb_text}' (no forge-{verb_text} script found)");
        return Ok(2);
    };

    let env = forge_env(&root, &config);
    let mut command = ProcessCommand::new(executable);
    command.args(&args[1..]);
    apply_env(&mut command, &env);

    let status = command
        .status()
        .map_err(|error| format!("cannot run forge {verb_text}: {error}"))?;
    Ok(status.code().unwrap_or(1))
}

pub(crate) fn forge_root_from(cwd: &Path) -> Result<PathBuf, String> {
    let absolute = absolutize(cwd)?;
    for candidate in absolute.ancestors() {
        if candidate.join("module.yaml").is_file() {
            return Ok(candidate.to_path_buf());
        }
    }
    Ok(absolute)
}

fn absolutize(path: &Path) -> Result<PathBuf, String> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    env::current_dir()
        .map(|cwd| cwd.join(path))
        .map_err(|error| error.to_string())
}

fn resolve_external(command_name: &str, root: &Path, extensions: &[PathBuf]) -> Option<PathBuf> {
    let local = root.join("commands").join(command_name);
    if local.is_file() {
        return Some(local);
    }
    for extension in extensions {
        let candidate = extension.join(command_name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    resolve_on_path(command_name)
}

fn resolve_on_path(command_name: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    env::split_paths(&path)
        .map(|dir| dir.join(command_name))
        .find(|candidate| candidate.is_file())
}

pub(crate) fn forge_env(
    root: &Path,
    config: &ontology::ResolvedConfig,
) -> Vec<(OsString, OsString)> {
    let mut env = vec![
        (
            OsString::from("FORGE_ROOT"),
            root.as_os_str().to_os_string(),
        ),
        (OsString::from("CI"), OsString::from("1")),
    ];
    env.extend(
        ontology::env_vars(config)
            .into_iter()
            .map(|(key, value)| (OsString::from(key), OsString::from(value))),
    );
    env
}

pub(crate) fn apply_env(command: &mut ProcessCommand, env: &[(OsString, OsString)]) {
    for (key, value) in env {
        command.env(key, value);
    }
}

#[cfg(test)]
mod tests;
