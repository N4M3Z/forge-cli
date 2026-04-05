use commands::result::ActionResult;
use std::path::Path;
use std::process::Command;

use crate::cli::config;

pub fn run_external_checks(module_root: &Path, result: &mut ActionResult) {
    let exclude_patterns = load_exclude_patterns(module_root);

    check_trailing_whitespace(module_root, &exclude_patterns, result);
    check_yaml_syntax(module_root, &exclude_patterns, result);
    check_json_syntax(module_root, &exclude_patterns, result);
    check_shellcheck(module_root, result);
    check_cargo(module_root, result);
    check_typescript(module_root, result);
    check_ruff(module_root, result);
    check_gitleaks(module_root, result);
    check_semgrep(module_root, result);
}

fn load_exclude_patterns(module_root: &Path) -> Vec<String> {
    let Ok(merged_config) = config::load_merged_config(module_root) else {
        return Vec::new();
    };
    commands::yaml::yaml_list(&merged_config, "validate.exclude")
        .map(|list| list.split(", ").map(String::from).collect())
        .unwrap_or_default()
}

fn check_trailing_whitespace(
    module_root: &Path,
    exclude_patterns: &[String],
    result: &mut ActionResult,
) {
    let text_files = find_text_files(module_root);
    if text_files.is_empty() {
        return;
    }

    let mut violations = Vec::new();
    for path in &text_files {
        if is_excluded(path, module_root, exclude_patterns) {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(path) else {
            continue;
        };
        for (line_number, line) in content.lines().enumerate() {
            if line != line.trim_end() {
                violations.push(format!("{}:{}", path.display(), line_number + 1));
                break;
            }
        }
    }

    if !violations.is_empty() {
        println!("  trailing whitespace ({} files)", violations.len());
        result
            .errors
            .push(format!("trailing whitespace in: {}", violations.join(", ")));
    }
}

fn check_yaml_syntax(module_root: &Path, exclude_patterns: &[String], result: &mut ActionResult) {
    let mut all_yaml = find_files(module_root, "yaml");
    all_yaml.extend(find_files(module_root, "yml"));
    if all_yaml.is_empty() {
        return;
    }

    for path in &all_yaml {
        if is_excluded(path, module_root, exclude_patterns) {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(path) else {
            continue;
        };
        if let Err(error) = serde_yaml::from_str::<serde_yaml::Value>(&content) {
            println!("  INVALID {}: {error}", path.display());
            result
                .errors
                .push(format!("invalid YAML: {}", path.display()));
        }
    }
}

fn check_json_syntax(module_root: &Path, exclude_patterns: &[String], result: &mut ActionResult) {
    let json_files = find_files(module_root, "json");
    if json_files.is_empty() {
        return;
    }

    for path in &json_files {
        if is_excluded(path, module_root, exclude_patterns) {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(path) else {
            continue;
        };
        if let Err(error) = serde_json::from_str::<serde_json::Value>(&content) {
            println!("  INVALID {}: {error}", path.display());
            result
                .errors
                .push(format!("invalid JSON: {}", path.display()));
        }
    }
}

fn is_excluded(path: &Path, module_root: &Path, patterns: &[String]) -> bool {
    let relative = path
        .strip_prefix(module_root)
        .unwrap_or(path)
        .to_string_lossy();
    patterns.iter().any(|pattern| {
        if let Some(prefix) = pattern.strip_suffix("/*") {
            relative.starts_with(prefix)
        } else {
            relative.as_ref() == pattern.as_str()
        }
    })
}

fn check_shellcheck(module_root: &Path, result: &mut ActionResult) {
    if !has_tool("shellcheck") {
        return;
    }

    let shell_files = find_files(module_root, "sh");
    if shell_files.is_empty() {
        return;
    }

    println!("  shellcheck");
    let mut arguments = vec!["-S", "warning"];
    let paths: Vec<String> = shell_files
        .iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect();
    for path in &paths {
        arguments.push(path);
    }

    if !run_command("shellcheck", &arguments, module_root) {
        result.errors.push("shellcheck found warnings".to_string());
    }
}

fn check_cargo(module_root: &Path, result: &mut ActionResult) {
    if !module_root.join("Cargo.toml").is_file() || !has_tool("cargo") {
        return;
    }

    println!("  cargo fmt --check");
    if !run_command("cargo", &["fmt", "--check"], module_root) {
        result
            .errors
            .push("cargo fmt found formatting issues".to_string());
    }

    println!("  cargo clippy");
    if !run_command("cargo", &["clippy", "--", "-D", "warnings"], module_root) {
        result
            .errors
            .push("cargo clippy found warnings".to_string());
    }
}

fn check_typescript(module_root: &Path, result: &mut ActionResult) {
    if !module_root.join("tsconfig.json").is_file() || !has_tool("npx") {
        return;
    }

    let typescript_files = find_files(module_root, "ts");
    if typescript_files.is_empty() {
        return;
    }

    println!("  tsc --noEmit");
    if !run_command("npx", &["tsc", "--noEmit"], module_root) {
        result.errors.push("tsc found type errors".to_string());
    }
}

fn check_ruff(module_root: &Path, result: &mut ActionResult) {
    if !has_tool("ruff") {
        return;
    }

    let python_files = find_files(module_root, "py");
    if python_files.is_empty() {
        return;
    }

    println!("  ruff check");
    if !run_command("ruff", &["check", "."], module_root) {
        result.errors.push("ruff found issues".to_string());
    }
}

fn check_semgrep(module_root: &Path, result: &mut ActionResult) {
    if !has_tool("semgrep") {
        return;
    }

    println!("  semgrep OWASP");
    if !run_command(
        "semgrep",
        &[
            "scan",
            "--config=p/owasp-top-ten",
            "--metrics=off",
            "--quiet",
            ".",
        ],
        module_root,
    ) {
        result.errors.push("semgrep found issues".to_string());
    }
}

fn check_gitleaks(module_root: &Path, result: &mut ActionResult) {
    if !has_tool("gitleaks") {
        return;
    }

    println!("  gitleaks detect");
    if !run_command(
        "gitleaks",
        &["detect", "--no-banner", "--no-git", "-s", "."],
        module_root,
    ) {
        result.errors.push("gitleaks found secrets".to_string());
    }
}

fn has_tool(name: &str) -> bool {
    Command::new("which")
        .arg(name)
        .output()
        .is_ok_and(|output| output.status.success())
}

fn run_command(program: &str, arguments: &[&str], working_directory: &Path) -> bool {
    Command::new(program)
        .args(arguments)
        .current_dir(working_directory)
        .status()
        .is_ok_and(|status| status.success())
}

fn find_text_files(module_root: &Path) -> Vec<std::path::PathBuf> {
    let text_extensions = [
        "md", "yaml", "yml", "toml", "json", "sh", "rs", "py", "ts", "tsx", "js",
    ];
    let mut files = Vec::new();
    for extension in &text_extensions {
        files.extend(find_files(module_root, extension));
    }
    files
}

fn find_files(module_root: &Path, extension: &str) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    collect_files_recursive(module_root, extension, &mut files);
    files
}

fn collect_files_recursive(directory: &Path, extension: &str, files: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name().unwrap_or_default().to_string_lossy();

        if name.starts_with('.') || name == "build" || name == "target" || name == "node_modules" {
            continue;
        }

        if path.is_dir() {
            if path.join(".git").exists() {
                continue;
            }
            collect_files_recursive(&path, extension, files);
        } else if path.extension().is_some_and(|found| found == extension) {
            files.push(path);
        }
    }
}
