use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Workspace,
    User,
    Project,
    Directory,
    All,
}

// Given provider_target=".claude", content_kind="rules":
//   Workspace  -> .claude/rules
//   User       -> /home/user/.claude/rules          (needs home_directory)
//   Project    -> /home/user/.claude/Users-me-X/rules (needs home_directory)
//   Directory  -> /Developer/blah/rules              (needs target_directory)
//   All        -> [.claude/rules, /home/user/.claude/rules] (needs home_directory)
pub fn resolve_paths(
    scope: Scope,
    provider_target: &str,
    content_kind: &str,
    home_directory: Option<&Path>,
    target_directory: Option<&Path>,
) -> Result<Vec<PathBuf>, String> {
    match scope {
        Scope::Workspace => {
            let path = PathBuf::from(provider_target).join(content_kind);
            Ok(vec![path])
        }
        Scope::User => {
            let home =
                home_directory.ok_or_else(|| "User scope requires home_directory".to_string())?;
            let path = home.join(provider_target).join(content_kind);
            Ok(vec![path])
        }
        Scope::Project => {
            let home = home_directory
                .ok_or_else(|| "Project scope requires home_directory".to_string())?;
            let cwd = std::env::current_dir().unwrap_or_default();
            let cwd_string = cwd.to_string_lossy().to_string();
            let project_key = cwd_string.replace('/', "-");
            let project_key = project_key.trim_start_matches('-');

            let path = home
                .join(provider_target)
                .join(project_key)
                .join(content_kind);
            Ok(vec![path])
        }
        Scope::Directory => {
            let target = target_directory
                .ok_or_else(|| "Directory scope requires target_directory".to_string())?;
            let path = target.join(content_kind);
            Ok(vec![path])
        }
        Scope::All => {
            let home =
                home_directory.ok_or_else(|| "All scope requires home_directory".to_string())?;
            let workspace_path = PathBuf::from(provider_target).join(content_kind);
            let user_path = home.join(provider_target).join(content_kind);
            Ok(vec![workspace_path, user_path])
        }
    }
}

#[cfg(test)]
mod tests;
