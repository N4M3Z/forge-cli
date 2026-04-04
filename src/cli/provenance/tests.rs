use super::*;
use std::path::Path;

#[test]
fn resolve_sidecar_path_appends_provenance_directory() {
    let result = resolve_sidecar_path(Path::new("/home/.claude/rules/UseRTK.md"));
    let result_string = result.to_string_lossy();
    assert!(result_string.contains(commands::manifest::PROVENANCE_DIRECTORY));
}

#[test]
fn resolve_sidecar_path_uses_stem_not_full_filename() {
    let result = resolve_sidecar_path(Path::new("/home/.claude/agents/Dev.md"));
    let filename = result.file_name().unwrap().to_string_lossy();
    assert!(!filename.contains(".md."));
    assert!(filename.starts_with("Dev."));
}

#[test]
fn resolve_sidecar_path_preserves_parent_directory() {
    let result = resolve_sidecar_path(Path::new("/project/.claude/rules/UseRTK.md"));
    assert!(result.starts_with("/project/.claude/rules"));
}
