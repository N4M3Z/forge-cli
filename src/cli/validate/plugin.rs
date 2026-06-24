use commands::result::ActionResult;
use commands::validate::{Diagnostic, validate_hooks_manifest, validate_json_manifest};
use std::fs;
use std::path::Path;

/// Validate Claude Code plugin scaffolding, if present.
///
/// Runs only when `.claude-plugin/plugin.json` exists, so non-plugin modules
/// are unaffected. Checks the plugin manifest, the optional marketplace
/// manifest, and `hooks/hooks.json` (including that every referenced hook
/// script exists and is executable).
pub fn check_plugin_scaffolding(module_root: &Path, result: &mut ActionResult) {
    let plugin_manifest = module_root.join(".claude-plugin/plugin.json");
    if !plugin_manifest.is_file() {
        return;
    }

    println!("  claude-plugin scaffolding");

    check_manifest(&plugin_manifest, result, validate_json_manifest);

    let marketplace_manifest = module_root.join(".claude-plugin/marketplace.json");
    if marketplace_manifest.is_file() {
        check_manifest(&marketplace_manifest, result, validate_json_manifest);
    }

    let hooks_manifest = module_root.join("hooks/hooks.json");
    if hooks_manifest.is_file() {
        check_hooks(module_root, &hooks_manifest, result);
    }
}

fn check_manifest(
    path: &Path,
    result: &mut ActionResult,
    validator: impl Fn(&str, &str) -> Vec<Diagnostic>,
) {
    let display_path = path.to_string_lossy().to_string();
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(read_error) => {
            result
                .errors
                .push(format!("{display_path}: cannot read: {read_error}"));
            return;
        }
    };
    push_diagnostics(validator(&content, &display_path), result);
}

fn check_hooks(module_root: &Path, hooks_manifest: &Path, result: &mut ActionResult) {
    let display_path = hooks_manifest.to_string_lossy().to_string();
    let content = match fs::read_to_string(hooks_manifest) {
        Ok(content) => content,
        Err(read_error) => {
            result
                .errors
                .push(format!("{display_path}: cannot read: {read_error}"));
            return;
        }
    };

    let (diagnostics, scripts) = validate_hooks_manifest(&content, &display_path);
    push_diagnostics(diagnostics, result);

    for script in scripts {
        check_hook_script(module_root, &display_path, &script, result);
    }
}

fn check_hook_script(
    module_root: &Path,
    manifest_path: &str,
    script: &str,
    result: &mut ActionResult,
) {
    let script_path = module_root.join(script);
    if !script_path.is_file() {
        result
            .errors
            .push(format!("{manifest_path}: hook script not found: {script}"));
        return;
    }
    if !is_executable(&script_path) {
        result.errors.push(format!(
            "{manifest_path}: hook script is not executable (chmod +x): {script}"
        ));
    }
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(path).is_ok_and(|metadata| metadata.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable(_path: &Path) -> bool {
    // The executable bit is not meaningful on non-unix targets; treat the
    // script as runnable so validation does not flag a phantom problem.
    true
}

fn push_diagnostics(diagnostics: Vec<Diagnostic>, result: &mut ActionResult) {
    for diagnostic in diagnostics {
        result.errors.push(format!(
            "{}: {} ({:?})",
            diagnostic.file, diagnostic.message, diagnostic.severity
        ));
    }
}
