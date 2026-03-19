use super::FileStatus;
use std::collections::HashMap;

/// Compare source file hashes from a provenance sidecar against current source hashes.
///
/// Returns Stale if any source file changed since the last build.
/// Returns Unchanged if all source hashes match.
pub fn check_sources(
    provenance_sources: &[(String, String)],
    current_sources: &[(String, String)],
) -> FileStatus {
    if provenance_sources.len() != current_sources.len() {
        return FileStatus::Stale;
    }

    let mut stored: HashMap<&str, &str> = HashMap::new();
    for (file, sha256) in provenance_sources {
        stored.insert(file.as_str(), sha256.as_str());
    }

    for (file, sha256) in current_sources {
        match stored.get(file.as_str()) {
            Some(stored_sha256) => {
                if *stored_sha256 != sha256.as_str() {
                    return FileStatus::Stale;
                }
            }
            None => return FileStatus::Stale,
        }
    }

    FileStatus::Unchanged
}
