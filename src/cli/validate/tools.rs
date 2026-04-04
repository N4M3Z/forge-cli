use commands::result::ActionResult;
use std::path::Path;
use std::process::Command;

pub fn run_external_checks(module_root: &Path, result: &mut ActionResult) {
    check_shellcheck(module_root, result);
    check_cargo(module_root, result);
    check_typescript(module_root, result);
    check_gitleaks(module_root, result);
    run_cargo_test(module_root, result);
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

fn run_cargo_test(module_root: &Path, result: &mut ActionResult) {
    if !module_root.join("Cargo.toml").is_file() || !has_tool("cargo") {
        return;
    }

    println!("  cargo test");
    if !run_command("cargo", &["test"], module_root) {
        result.errors.push("cargo test failed".to_string());
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

fn check_gitleaks(module_root: &Path, result: &mut ActionResult) {
    if !has_tool("gitleaks") {
        return;
    }

    let has_staged_changes = Command::new("git")
        .args(["diff", "--cached", "--quiet"])
        .current_dir(module_root)
        .status()
        .is_ok_and(|status| !status.success());

    if has_staged_changes {
        println!("  gitleaks protect --staged");
        if !run_command(
            "gitleaks",
            &["protect", "--staged", "--no-banner"],
            module_root,
        ) {
            result
                .errors
                .push("gitleaks found secrets in staged changes".to_string());
        }
    } else {
        println!("  gitleaks detect");
        if !run_command(
            "gitleaks",
            &["detect", "--no-banner", "--no-git", "-s", "."],
            module_root,
        ) {
            result.errors.push("gitleaks found secrets".to_string());
        }
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
