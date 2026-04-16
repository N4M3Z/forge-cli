use commands::error::{Error, ErrorKind};
use commands::module;
use commands::result::{ActionResult, DeployedFile};
use std::fs;
use std::path::Path;
use std::process::Command;

use super::install;
use crate::cli::config;

const MAKEFILE_TEMPLATE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/templates/make/dist.mk"
));

/// Assemble, install to a staging directory, then package each provider's
/// output as a self-contained release tarball in `dist/`.
///
/// ```text
/// module/
///   build/staging/.claude/...   ← install output (with .manifest)
///   dist/{name}-claude-v{version}.tar.gz
/// ```
///
/// Each tarball wraps `.{provider}/` (with `.manifest` inside, written by
/// install), a generated `Makefile`, and the module `README.md`.
pub fn execute(path: &str, embed: bool) -> Result<ActionResult, Error> {
    let module_root = Path::new(path);
    let module_manifest = module::load(module_root).map_err(|error| {
        Error::new(
            ErrorKind::Config,
            format!("cannot load module.yaml: {error}"),
        )
    })?;

    // Stage everything via install (assemble + deploy + .manifest)
    let staging_dir = module_root.join("build").join("staging");
    let _ = fs::remove_dir_all(&staging_dir);
    let mut result = install::execute(
        path,
        Some(&staging_dir.to_string_lossy()),
        true,
        false,
        false,
    )?;
    result.installed.clear();
    result.skipped.clear();

    let merged_config = config::load_merged_config(module_root)?;
    let providers = config::load_providers(&merged_config)?;
    let readme_path = module_root.join("README.md");
    let dist_dir = module_root.join("dist");
    fs::create_dir_all(&dist_dir).map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot create {}: {error}", dist_dir.display()),
        )
    })?;

    for (provider_name, provider_config) in &providers {
        let staged_provider = staging_dir.join(&provider_config.target);
        if !staged_provider.is_dir() {
            continue;
        }

        let wrapper_name = format!(
            "{}-{provider_name}-v{}",
            module_manifest.name, module_manifest.version
        );
        let wrapper_dir = module_root.join("build").join(&wrapper_name);
        let _ = fs::remove_dir_all(&wrapper_dir);
        fs::create_dir_all(&wrapper_dir).map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!("cannot create {}: {error}", wrapper_dir.display()),
            )
        })?;

        // Move installed provider tree (including .manifest) into wrapper
        let dotfolder = wrapper_dir.join(&provider_config.target);
        fs::rename(&staged_provider, &dotfolder).map_err(|error| {
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

        // Tar to dist/ and clean staging
        let tarball_path = dist_dir.join(format!("{wrapper_name}.tar.gz"));
        create_tarball(
            &module_root.join("build"),
            &tarball_path,
            &wrapper_name,
            provider_name,
        )?;
        let _ = fs::remove_dir_all(&wrapper_dir);

        result.installed.push(DeployedFile {
            source: provider_config.target.clone(),
            target: tarball_path.to_string_lossy().to_string(),
            provider: provider_name.clone(),
        });
    }

    let _ = fs::remove_dir_all(&staging_dir);

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
