//! Schema and parser for `.forge`.

use commands::error::{Error, ErrorKind};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::PathBuf;

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DotForge {
    pub version: u32,
    pub sources: BTreeMap<String, Source>,
    #[serde(default)]
    pub artifacts: BTreeMap<String, ArtifactList>,
}

/// Where to find a producer module on disk. Currently only `Local`; a `Git`
/// variant is reserved for a follow-up issue. `untagged` so a YAML entry
/// like `{ path: ../forge-core }` parses without a redundant discriminator.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, untagged)]
pub enum Source {
    Local { path: PathBuf },
}

/// Per-source list of requested artifact names. Each kind defaults to empty
/// so `.forge` can request only one kind per source.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct ArtifactList {
    pub skills: Vec<String>,
    pub agents: Vec<String>,
    pub rules: Vec<String>,
}

impl ArtifactList {
    pub fn is_empty(&self) -> bool {
        self.skills.is_empty() && self.agents.is_empty() && self.rules.is_empty()
    }
}

pub fn parse(content: &str) -> Result<DotForge, Error> {
    let manifest: DotForge = serde_yaml::from_str(content)
        .map_err(|error| Error::new(ErrorKind::Parse, format!(".forge: {error}")))?;

    if manifest.version != SCHEMA_VERSION {
        return Err(Error::new(
            ErrorKind::Parse,
            format!(
                ".forge: schema version {} is not supported (this build only understands version {})",
                manifest.version, SCHEMA_VERSION
            ),
        ));
    }

    for source_label in manifest.artifacts.keys() {
        if !manifest.sources.contains_key(source_label) {
            return Err(Error::new(
                ErrorKind::Parse,
                format!(".forge: artifacts entry '{source_label}' has no matching `sources` entry"),
            ));
        }
    }

    Ok(manifest)
}
