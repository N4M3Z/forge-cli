use super::{FileStatus, ManifestEntry, content_sha256};

/// Determine deployment status by comparing target file against `.manifest`.
///
///   no manifest entry                              → New
///   target missing                                 → New
///   target hash != manifest hash                   → Modified (user edited)
///   target hash == manifest hash == build hash     → Unchanged
///   target hash == manifest hash != build hash     → Stale (source changed)
pub fn status(
    target_content: Option<&str>,
    manifest_entry: Option<&ManifestEntry>,
    build_sha256: &str,
) -> FileStatus {
    let Some(entry) = manifest_entry else {
        return FileStatus::New;
    };

    let Some(content) = target_content else {
        return FileStatus::New;
    };

    let target_sha256 = content_sha256(content);

    if target_sha256 != entry.sha256 {
        return FileStatus::Modified;
    }

    if entry.sha256 == build_sha256 {
        return FileStatus::Unchanged;
    }

    FileStatus::Stale
}
