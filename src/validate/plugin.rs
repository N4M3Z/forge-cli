//! Format-resilient checks for Claude Code plugin scaffolding.
//!
//! Deliberately schema-light: it confirms each manifest is valid JSON and
//! returns the hook scripts referenced via `${CLAUDE_PLUGIN_ROOT}` so the CLI
//! can check they exist and are executable (the most common "hooks don't fire"
//! cause). It does NOT assert the plugin/marketplace field shape, so it does
//! not break when Claude Code's plugin schema changes.

use crate::validate::{Diagnostic, Severity};
use regex::Regex;
use serde_json::Value;
use std::sync::OnceLock;

fn error(file: &str, message: String) -> Diagnostic {
    Diagnostic {
        file: file.to_string(),
        line: None,
        severity: Severity::Error,
        message,
    }
}

/// Confirm a plugin manifest (`plugin.json` or `marketplace.json`) is valid
/// JSON. No field-shape assertions: a corrupt manifest is the failure mode
/// worth catching, and the schema is Claude Code's to define.
pub fn validate_json_manifest(content: &str, file: &str) -> Vec<Diagnostic> {
    match serde_json::from_str::<Value>(content) {
        Ok(_) => Vec::new(),
        Err(parse_error) => vec![error(file, format!("invalid JSON: {parse_error}"))],
    }
}

/// Confirm `hooks/hooks.json` is valid JSON and return the hook script paths it
/// references via `${CLAUDE_PLUGIN_ROOT}/<path>`, for the CLI to check for
/// existence and the executable bit. Commands that do not use the
/// `${CLAUDE_PLUGIN_ROOT}` convention are left alone — the check is
/// opportunistic, not an assertion about command shape.
pub fn validate_hooks_manifest(content: &str, file: &str) -> (Vec<Diagnostic>, Vec<String>) {
    let parsed: Value = match serde_json::from_str(content) {
        Ok(value) => value,
        Err(parse_error) => {
            return (
                vec![error(file, format!("invalid JSON: {parse_error}"))],
                Vec::new(),
            );
        }
    };

    let scripts = collect_commands(&parsed)
        .iter()
        .filter_map(|command| extract_script_path(command))
        .collect();

    (Vec::new(), scripts)
}

/// Collect every string under a `command` key anywhere in the JSON tree.
/// `hooks.json` nests commands under `hooks` → event → matcher groups, so a
/// recursive sweep reaches them without hard-coding the event names.
fn collect_commands(value: &Value) -> Vec<String> {
    let mut commands = Vec::new();
    walk_commands(value, &mut commands);
    commands
}

fn walk_commands(value: &Value, commands: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                if key == "command"
                    && let Value::String(command) = child
                {
                    commands.push(command.clone());
                }
                walk_commands(child, commands);
            }
        }
        Value::Array(items) => {
            for item in items {
                walk_commands(item, commands);
            }
        }
        _ => {}
    }
}

/// Extract the script path that follows `${CLAUDE_PLUGIN_ROOT}/` in a command,
/// or `None` if the command does not use the convention.
fn extract_script_path(command: &str) -> Option<String> {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    let regex = PATTERN.get_or_init(|| {
        Regex::new(r#"\$\{CLAUDE_PLUGIN_ROOT\}/([^"\s]+)"#).expect("static plugin-root regex")
    });
    regex
        .captures(command)
        .and_then(|capture| capture.get(1))
        .map(|matched| matched.as_str().to_string())
}
