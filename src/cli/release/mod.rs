use commands::error::{Error, ErrorKind};
use commands::result::{ActionResult, DeployedFile};
use std::fs;
use std::path::Path;
use std::process::Command;

use super::assemble;
use crate::cli::config;

/// Assemble and package the module as release tarballs.
///
/// Steps:
///   1. Run `assemble` to populate build/
///   2. Create a `.tar.gz` for each provider directory in build/
///   3. If `--embed` is requested, print a not-yet-implemented message
///
/// ```text
/// module/build/
///   claude/...     → module-claude.tar.gz
///   gemini/...     → module-gemini.tar.gz
/// ```
pub fn execute(path: &str, embed: bool) -> Result<ActionResult, Error> {
    let mut result = assemble::execute(path)?;

    let module_root = Path::new(path);
    let build_dir = module_root.join("build");

    if !build_dir.is_dir() {
        return Ok(result);
    }

    let merged_config = config::load_merged_config(module_root)?;
    let providers = config::load_providers(&merged_config)?;

    for provider_name in providers.keys() {
        let provider_dir = build_dir.join(provider_name);
        if !provider_dir.is_dir() {
            continue;
        }

        let tarball_name = format!("module-{provider_name}.tar.gz");
        let tarball_path = build_dir.join(&tarball_name);

        create_tarball(&provider_dir, &tarball_path, provider_name)?;

        result.installed.push(DeployedFile {
            source: provider_dir.to_string_lossy().to_string(),
            target: tarball_path.to_string_lossy().to_string(),
            provider: provider_name.clone(),
        });
    }

    if embed {
        eprintln!("warning: --embed is not yet implemented");
    }

    Ok(result)
}

/// Create a .tar.gz archive of a provider's build directory.
///
/// Uses the system `tar` command:
///
/// ```text
/// tar -czf module-claude.tar.gz -C build/claude .
/// ```
fn create_tarball(
    source_dir: &Path,
    tarball_path: &Path,
    provider_name: &str,
) -> Result<(), Error> {
    // Ensure parent directory exists
    if let Some(parent) = tarball_path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            Error::new(
                ErrorKind::Io,
                format!("cannot create {}: {e}", parent.display()),
            )
        })?;
    }

    let output = Command::new("tar")
        .args([
            "-czf",
            &tarball_path.to_string_lossy(),
            "-C",
            &source_dir.to_string_lossy(),
            ".",
        ])
        .output()
        .map_err(|e| {
            Error::new(
                ErrorKind::Io,
                format!("failed to run tar for {provider_name}: {e}"),
            )
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::new(
            ErrorKind::Io,
            format!("tar failed for {provider_name}: {stderr}"),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests;
