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
    let Source::Local { path } = source;
    let resolved = if path.is_absolute() {
        path.clone()
    } else {
        repo_root.join(path)
    };
    let canonical = fs::canonicalize(&resolved).map_err(|error| {
        Error::new(
            ErrorKind::Config,
            format!(
                ".forge: source '{source_label}' path {} does not exist: {error}",
                resolved.display()
            ),
        )
    })?;
    if !canonical.join("module.yaml").is_file() {
        return Err(Error::new(
            ErrorKind::Config,
            format!(
                ".forge: source '{source_label}' at {} has no module.yaml",
                canonical.display()
            ),
        ));
    }
    Ok(canonical)
}
