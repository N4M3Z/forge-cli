pub(crate) mod extract;
mod read;
mod staleness;
mod statement;
mod status;
mod write;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub use read::read;
pub use staleness::check_sources;
pub use statement::generate_statement;
pub use status::status;
pub use write::write;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileStatus {
    New,
    Unchanged,
    Stale,
    Modified,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestEntry {
    pub fingerprint: String,
    pub provenance: Option<String>,
}

pub const SIDECAR_EXTENSION: &str = "yaml";
pub const PROVENANCE_DIRECTORY: &str = ".provenance";

/// Compute the provenance sidecar path relative to the provider target.
///
/// `rules/CurrencyFormatting.md` → `rules/.provenance/CurrencyFormatting.yaml`
/// `rules/cz/PersonalTaxIncome.md` → `rules/cz/.provenance/PersonalTaxIncome.yaml`
/// `skills/SessionPrep/SKILL.md` → `skills/SessionPrep/.provenance/SKILL.yaml`
pub fn provenance_path(manifest_key: &str) -> String {
    let path = std::path::Path::new(manifest_key);
    let parent = path.parent().unwrap_or(std::path::Path::new(""));
    let stem = path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy();

    let provenance_dir = parent.join(PROVENANCE_DIRECTORY);
    provenance_dir
        .join(format!("{stem}.{SIDECAR_EXTENSION}"))
        .to_string_lossy()
        .to_string()
}

/// Compute the build sidecar path from a content file path.
pub fn sidecar_path(content_path: &std::path::Path) -> std::path::PathBuf {
    content_path.with_extension(SIDECAR_EXTENSION)
}

pub fn content_sha256(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests;
