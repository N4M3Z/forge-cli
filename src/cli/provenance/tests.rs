use super::*;
use std::path::Path;
use tempfile::TempDir;

#[test]
fn resolve_sidecar_path_appends_provenance_directory() {
    let result = resolve_sidecar_path(Path::new("/home/.claude/rules/UseRTK.md"));
    let result_string = result.to_string_lossy();
    assert!(result_string.contains(commands::manifest::PROVENANCE_DIRECTORY));
}

#[test]
fn resolve_sidecar_path_uses_stem_when_neither_exists() {
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

#[test]
fn resolve_sidecar_path_prefers_extensionless_when_both_absent() {
    let temp = TempDir::new().unwrap();
    let file_path = temp.path().join("agents/Dev.md");
    std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
    std::fs::write(&file_path, "body").unwrap();

    let result = resolve_sidecar_path(&file_path);
    assert!(result.ends_with("agents/.provenance/Dev.yaml"));
}

#[test]
fn resolve_sidecar_path_falls_back_to_extension_preserving() {
    let temp = TempDir::new().unwrap();
    let file_path = temp.path().join("skills/CodeCleanup/CleanCode.md");
    std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
    std::fs::write(&file_path, "body").unwrap();

    let provenance_dir = file_path.parent().unwrap().join(".provenance");
    std::fs::create_dir_all(&provenance_dir).unwrap();
    let extension_preserving = provenance_dir.join("CleanCode.md.yaml");
    std::fs::write(&extension_preserving, "stub").unwrap();

    let result = resolve_sidecar_path(&file_path);
    assert_eq!(result, extension_preserving);
}

#[test]
fn resolve_sidecar_path_prefers_extensionless_when_both_exist() {
    let temp = TempDir::new().unwrap();
    let file_path = temp.path().join("agents/Dev.md");
    std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
    std::fs::write(&file_path, "body").unwrap();

    let provenance_dir = file_path.parent().unwrap().join(".provenance");
    std::fs::create_dir_all(&provenance_dir).unwrap();
    let extensionless = provenance_dir.join("Dev.yaml");
    std::fs::write(&extensionless, "stub").unwrap();
    std::fs::write(provenance_dir.join("Dev.md.yaml"), "stub").unwrap();

    let result = resolve_sidecar_path(&file_path);
    assert_eq!(
        result, extensionless,
        "extension-less sidecar should win when both exist"
    );
}
