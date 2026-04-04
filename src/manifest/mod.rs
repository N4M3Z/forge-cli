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
    pub sha256: String,
}

pub fn content_sha256(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests;
