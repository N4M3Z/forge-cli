use commands::error::{Error, ErrorKind};
use std::fs;
use std::path::Path;

use crate::cli::config::read_file;

/// A source file discovered during directory walking.
///
/// ```text
/// SourceFile {
///     relative_path: "rules/MyRule.md",
///     full_path: "/home/user/module/rules/MyRule.md",
///     content: "---\nname: MyRule\n---\n...",
///     kind: "rules",
///     passthrough: false,
/// }
/// ```
pub struct SourceFile {
    /// Relative path from the module root (e.g. "rules/MyRule.md").
    pub relative_path: String,
    /// Full filesystem path.
    pub full_path: String,
    /// Raw file content.
    pub content: String,
    /// Content kind: "agents", "skills", or "rules".
    pub kind: String,
    /// Whether this file is a passthrough (non-SKILL.md file inside a skill dir).
    pub passthrough: bool,
}

/// Walk agents/, skills/, rules/ and collect all .md source files.
///
/// Given a module root like:
///
/// ```text
/// module/
///   agents/SecurityArchitect.md
///   rules/MyRule.md
///   skills/Explain/SKILL.md
///   skills/Explain/examples.md
/// ```
///
/// Returns `SourceFiles` for each .md file. For skills, SKILL.md files
/// are marked `passthrough: false` (assembled), other .md files in
/// skill directories are `passthrough: true` (copied verbatim).
pub fn collect(module_root: &Path) -> Result<Vec<SourceFile>, Error> {
    let mut sources = Vec::new();

    for kind in &["agents", "skills", "rules"] {
        let dir = module_root.join(kind);
        if !dir.is_dir() {
            continue;
        }

        walk_content_dir(&dir, kind, module_root, &mut sources)?;
    }

    Ok(sources)
}

/// Walk a flat content directory (agents/ or rules/), collecting .md files.
fn walk_content_dir(
    dir: &Path,
    kind: &str,
    module_root: &Path,
    sources: &mut Vec<SourceFile>,
) -> Result<(), Error> {
    let entries = fs::read_dir(dir)
        .map_err(|e| Error::new(ErrorKind::Io, format!("cannot read {}: {e}", dir.display())))?;

    for entry in entries {
        let entry =
            entry.map_err(|e| Error::new(ErrorKind::Io, format!("directory entry error: {e}")))?;

        let path = entry.path();

        if path.is_dir() {
            if kind == "skills" {
                walk_skill_dir(&path, kind, module_root, sources)?;
            }
            continue;
        }

        if path.extension().unwrap_or_default() != "md" {
            continue;
        }

        let relative = path
            .strip_prefix(module_root)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string();

        let content = read_file(&path)?;

        sources.push(SourceFile {
            relative_path: relative,
            full_path: path.to_string_lossy().to_string(),
            content,
            kind: kind.to_string(),
            passthrough: false,
        });
    }

    Ok(())
}

/// Walk a skill subdirectory. SKILL.md is assembled; other .md files are passthrough.
///
/// Given `skills/Explain/`:
///
/// ```text
/// skills/Explain/SKILL.md       → passthrough: false (assembled)
/// skills/Explain/examples.md    → passthrough: true  (copied verbatim)
/// ```
fn walk_skill_dir(
    dir: &Path,
    kind: &str,
    module_root: &Path,
    sources: &mut Vec<SourceFile>,
) -> Result<(), Error> {
    let entries = fs::read_dir(dir)
        .map_err(|e| Error::new(ErrorKind::Io, format!("cannot read {}: {e}", dir.display())))?;

    for entry in entries {
        let entry =
            entry.map_err(|e| Error::new(ErrorKind::Io, format!("directory entry error: {e}")))?;

        let path = entry.path();

        if path.is_dir() || path.extension().unwrap_or_default() != "md" {
            continue;
        }

        let filename = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let is_skill_file = filename == "SKILL.md";

        let relative = path
            .strip_prefix(module_root)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string();

        let content = read_file(&path)?;

        sources.push(SourceFile {
            relative_path: relative,
            full_path: path.to_string_lossy().to_string(),
            content,
            kind: kind.to_string(),
            passthrough: !is_skill_file,
        });
    }

    Ok(())
}
