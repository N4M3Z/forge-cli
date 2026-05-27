//! Walk each declared source module on disk and feed its content through
//! the artifact filter. The output flat `Vec<SourceFile>` plugs straight
//! into the existing per-provider assemble loop.

use commands::error::{Error, ErrorKind};
use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::cli::assemble::sources::{self, SourceFile};
use crate::cli::dotforge::filter::filter_to_requested;
use crate::cli::dotforge::parse::{DotForge, Source};

pub fn resolve_sources(
    manifest: &DotForge,
    repo_root: &Path,
    valid_qualifiers: &HashSet<String>,
) -> Result<Vec<SourceFile>, Error> {
    let mut collected: Vec<SourceFile> = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();

    for (source_label, source) in &manifest.sources {
        let canonical = canonicalize_source(source, source_label, repo_root)?;
        let Some(artifact_list) = manifest.artifacts.get(source_label) else {
            continue;
        };
        if artifact_list.is_empty() {
            continue;
        }

        let all_files = sources::collect(&canonical, valid_qualifiers)?;
        let filtered = filter_to_requested(all_files, artifact_list, source_label, &canonical)?;

        for file in filtered {
            if !seen.insert(file.relative_path.clone()) {
                return Err(Error::new(
                    ErrorKind::Config,
                    format!(
                        ".forge: artifact {} requested from more than one source",
                        file.relative_path
                    ),
                ));
            }
            collected.push(file);
        }
    }

    Ok(collected)
}

fn canonicalize_source(
    source: &Source,
    source_label: &str,
    repo_root: &Path,
) -> Result<PathBuf, Error> {
    let materialized = match source {
        Source::Local { path } => canonicalize_local(path, source_label, repo_root)?,
        Source::Git { git, commit } => {
            crate::cli::dotforge::git::ensure_cached(git, commit, source_label)?
        }
    };
    if !materialized.join("module.yaml").is_file() {
        return Err(Error::new(
            ErrorKind::Config,
            format!(
                ".forge: source '{source_label}' at {} has no module.yaml",
                materialized.display()
            ),
        ));
    }
    Ok(materialized)
}

fn canonicalize_local(path: &Path, source_label: &str, repo_root: &Path) -> Result<PathBuf, Error> {
    let resolved = if path.is_absolute() {
        path.to_path_buf()
    } else {
        repo_root.join(path)
    };
    fs::canonicalize(&resolved).map_err(|error| {
        Error::new(
            ErrorKind::Config,
            format!(
                ".forge: source '{source_label}' path {} does not exist: {error}",
                resolved.display()
            ),
        )
    })
}
