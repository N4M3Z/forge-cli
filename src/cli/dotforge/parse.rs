//! Schema and parser for `.forge`.

use commands::error::{Error, ErrorKind};
use serde::{Deserialize, Deserializer};
use std::collections::BTreeMap;
use std::path::PathBuf;

pub const SCHEMA_VERSION: u32 = 1;

/// Test-only escape hatch: when this env var is set, `file://` URLs pass
/// validation so integration tests can use a local bare repo as the origin
/// without spinning up an HTTPS server. Never set in production: the
/// HTTPS-only rule defends against `git://` MITM and accidental local-path
/// pulls that bypass SHA pinning.
const ALLOW_FILE_URLS_ENV: &str = "FORGE_GIT_ALLOW_FILE_URLS";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DotForge {
    pub version: u32,
    pub sources: BTreeMap<String, Source>,
    #[serde(default)]
    pub artifacts: BTreeMap<String, ArtifactList>,
}

/// Where to find a producer module. `Local` for a sibling checkout on disk,
/// `Git` for a remote HTTPS repository pinned to a 40-hex commit SHA.
///
/// A custom `Deserialize` picks the variant by which key is present (`path:`
/// vs `git:`) so per-variant validation errors propagate cleanly to the
/// user. `serde(untagged)` would swallow them into a generic "did not match
/// any variant" message.
#[derive(Debug)]
pub enum Source {
    Local { path: PathBuf },
    Git { git: String, commit: String },
}

impl<'de> Deserialize<'de> for Source {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error as _;
        let value: serde_yaml::Value = Deserialize::deserialize(deserializer)?;
        let mapping = value.as_mapping().ok_or_else(|| {
            D::Error::custom(
                "source entry must be a mapping (e.g. `path: ../foo` or `git: https://...`)",
            )
        })?;
        let has_path = mapping.contains_key(serde_yaml::Value::from("path"));
        let has_git = mapping.contains_key(serde_yaml::Value::from("git"));
        match (has_path, has_git) {
            (true, true) => Err(D::Error::custom(
                "source entry cannot have both `path` and `git`; pick one",
            )),
            (true, false) => {
                let local: LocalFields = serde_yaml::from_value(value).map_err(D::Error::custom)?;
                Ok(Source::Local { path: local.path })
            }
            (false, true) => {
                let git: GitFields = serde_yaml::from_value(value).map_err(D::Error::custom)?;
                validate_git_url(&git.git).map_err(D::Error::custom)?;
                validate_commit_sha(&git.commit).map_err(D::Error::custom)?;
                Ok(Source::Git {
                    git: git.git,
                    commit: git.commit,
                })
            }
            (false, false) => Err(D::Error::custom(
                "source entry must contain either `path:` (local checkout) or `git:` (remote URL)",
            )),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LocalFields {
    path: PathBuf,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GitFields {
    git: String,
    #[serde(rename = "ref")]
    commit: String,
}

pub fn validate_git_url(url: &str) -> Result<(), String> {
    let allow_file = std::env::var(ALLOW_FILE_URLS_ENV).is_ok();
    if allow_file && url.starts_with("file://") {
        return Ok(());
    }
    let Some(after_scheme) = url.strip_prefix("https://") else {
        return Err(format!("git URL must start with https://, got: {url}"));
    };
    let host_segment = after_scheme.split('/').next().unwrap_or_default();
    if host_segment.contains('@') {
        return Err(format!(
            "git URL must not embed user@ credentials in the host: {url}"
        ));
    }
    if host_segment.is_empty() {
        return Err(format!("git URL has no host: {url}"));
    }
    Ok(())
}

pub fn validate_commit_sha(sha: &str) -> Result<(), String> {
    if sha.len() != 40 {
        return Err(format!(
            "ref must be a 40-char commit SHA; got {} chars: {sha}",
            sha.len()
        ));
    }
    if !sha
        .chars()
        .all(|character| character.is_ascii_digit() || ('a'..='f').contains(&character))
    {
        return Err(format!(
            "ref must be 40 lowercase hex chars (no branch names, tags, or uppercase): {sha}"
        ));
    }
    Ok(())
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
