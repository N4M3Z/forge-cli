use commands::error::{Error, ErrorKind};
use commands::result::{ActionResult, DeployedFile};
use std::fs;
use std::path::Path;
use std::process::Command;

use super::assemble;
use crate::cli::config;
use commands::module;

const MAKEFILE_TEMPLATE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/templates/dist/Makefile"
));

/// Assemble and package the module as self-contained release tarballs.
///
/// ```text
/// module/build/
///   claude/...     → forge-core-claude-v0.5.0.tar.gz
///   gemini/...     → forge-core-gemini-v0.5.0.tar.gz
/// ```
pub fn execute(path: &str, embed: bool) -> Result<ActionResult, Error> {
    let mut result = assemble::execute(path)?;

    let module_root = Path::new(path);
    let build_dir = module_root.join("build");

    if !build_dir.is_dir() {
        return Ok(result);
    }

    let manifest = module::load(module_root).map_err(|error| {
        Error::new(
            ErrorKind::Config,
            format!("cannot load module.yaml: {error}"),
        )
    })?;

    let merged_config = config::load_merged_config(module_root)?;
    let providers = config::load_providers(&merged_config)?;
    let readme_path = module_root.join("README.md");

    for provider_name in providers.keys() {
        let provider_dir = build_dir.join(provider_name);
        if !provider_dir.is_dir() {
            continue;
        }

        let wrapper_name = format!("{}-{provider_name}-v{}", manifest.name, manifest.version);
        let wrapper_dir = build_dir.join(&wrapper_name);
        let dotfolder = wrapper_dir.join(format!(".{provider_name}"));

        // Move assembled content into .{provider}/ inside wrapper
        fs::create_dir_all(&wrapper_dir).map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!("cannot create {}: {error}", wrapper_dir.display()),
            )
        })?;
        fs::rename(&provider_dir, &dotfolder).map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!("cannot move {provider_name}: {error}"),
            )
        })?;

        // Add Makefile and README
        let makefile_content = MAKEFILE_TEMPLATE.replace("${PROVIDER}", provider_name);
        fs::write(wrapper_dir.join("Makefile"), makefile_content).map_err(|error| {
            Error::new(ErrorKind::Io, format!("cannot write Makefile: {error}"))
        })?;
        if readme_path.is_file() {
            let _ = fs::copy(&readme_path, wrapper_dir.join("README.md"));
        }

        // Tar and clean up
        let tarball_path = build_dir.join(format!("{wrapper_name}.tar.gz"));
        create_tarball(&build_dir, &tarball_path, &wrapper_name, provider_name)?;
        let _ = fs::remove_dir_all(&wrapper_dir);

        result.installed.push(DeployedFile {
            source: format!(".{provider_name}"),
            target: tarball_path.to_string_lossy().to_string(),
            provider: provider_name.clone(),
        });
    }

    if embed {
        eprintln!("warning: --embed is not yet implemented");
    }

    Ok(result)
}

fn create_tarball(
    parent_dir: &Path,
    tarball_path: &Path,
    wrapper_name: &str,
    provider_name: &str,
) -> Result<(), Error> {
    let output = Command::new("tar")
        .args([
            "-czf",
            &tarball_path.to_string_lossy(),
            "-C",
            &parent_dir.to_string_lossy(),
            wrapper_name,
        ])
        .output()
        .map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!("tar failed for {provider_name}: {error}"),
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
