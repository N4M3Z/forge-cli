use super::*;
use std::path::{Path, PathBuf};

const PROVIDER: &str = ".claude";
const KIND: &str = "rules";

fn home() -> &'static Path {
    Path::new("/home/user")
}

fn target_dir() -> &'static Path {
    Path::new("/Developer/blah")
}

#[test]
fn workspace_scope_produces_relative_path() {
    let paths = resolve_paths(Scope::Workspace, PROVIDER, KIND, None, None);

    let expected = vec![PathBuf::from(".claude/rules")];
    assert_eq!(paths, expected);
}

#[test]
fn user_scope_produces_path_under_home() {
    let paths = resolve_paths(Scope::User, PROVIDER, KIND, Some(home()), None);

    let expected = vec![PathBuf::from("/home/user/.claude/rules")];
    assert_eq!(paths, expected);
}

#[test]
fn directory_scope_uses_target_directory() {
    let paths = resolve_paths(Scope::Directory, PROVIDER, KIND, None, Some(target_dir()));

    let expected = vec![PathBuf::from("/Developer/blah/rules")];
    assert_eq!(paths, expected);
}

#[test]
fn all_scope_returns_workspace_and_user() {
    let paths = resolve_paths(Scope::All, PROVIDER, KIND, Some(home()), None);

    let expected = vec![
        PathBuf::from(".claude/rules"),
        PathBuf::from("/home/user/.claude/rules"),
    ];
    assert_eq!(paths, expected);
}

#[test]
fn project_scope_includes_cwd_based_key() {
    let paths = resolve_paths(Scope::Project, PROVIDER, "agents", Some(home()), None);

    assert_eq!(paths.len(), 1);
    let path_string = paths[0].to_string_lossy().to_string();
    assert!(path_string.starts_with("/home/user/.claude/"));
    assert!(path_string.ends_with("/agents"));
}

#[test]
fn workspace_ignores_home_and_target() {
    let paths = resolve_paths(
        Scope::Workspace,
        PROVIDER,
        KIND,
        Some(home()),
        Some(target_dir()),
    );

    let expected = vec![PathBuf::from(".claude/rules")];
    assert_eq!(paths, expected);
}

#[test]
fn arbitrary_provider_and_content_kind() {
    let paths = resolve_paths(
        Scope::User,
        ".custom-provider",
        "templates",
        Some(home()),
        None,
    );

    let expected = vec![PathBuf::from("/home/user/.custom-provider/templates")];
    assert_eq!(paths, expected);
}
