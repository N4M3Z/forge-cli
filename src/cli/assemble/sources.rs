use commands::error::{Error, ErrorKind};
use std::collections::HashSet;
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
///     kind: ContentKind::Rules,
///     passthrough: false,
///     qualifier: None,
/// }
/// ```
pub struct SourceFile {
    /// Relative path from the module root (e.g. "rules/MyRule.md").
    pub relative_path: String,
    /// Full filesystem path.
    pub full_path: String,
    /// Raw file content.
    pub content: String,
    pub kind: commands::provider::ContentKind,
    /// Whether this file is a passthrough (non-SKILL.md file inside a skill dir).
    pub passthrough: bool,
    /// Qualifier directory name (e.g., "sonnet", "codex"), or None for base files.
    pub qualifier: Option<String>,
}

/// Walk agents/, skills/, rules/ and collect all .md source files.
///
/// Given a module root like:
///
/// ```text
/// module/
///   agents/SecurityArchitect.md
///   rules/MyRule.md
///   rules/sonnet/ReviewDiscipline.md   (qualifier-only)
///   rules/codex/AgentTeams.md          (variant override)
///   skills/Explain/SKILL.md
///   skills/Explain/examples.md
/// ```
///
/// Returns `SourceFiles` for each .md file. For skills, SKILL.md files
/// are marked `passthrough: false` (assembled), other .md files in
/// skill directories are `passthrough: true` (copied verbatim).
///
/// For rules and agents, subdirectories matching valid qualifier names
/// are walked for qualifier-only files (files with no base counterpart).
/// The `user/` directory is skipped here (handled by variant resolution).
/// Skills do not support qualifier directories.
pub fn collect(
    module_root: &Path,
    valid_qualifiers: &HashSet<String>,
) -> Result<Vec<SourceFile>, Error> {
    let mut sources = Vec::new();

    for kind in commands::provider::ContentKind::ALL {
        let dir = module_root.join(kind.as_str());
        if !dir.is_dir() {
            continue;
        }

        let qualifiers = if *kind == commands::provider::ContentKind::Skills {
            &HashSet::new()
        } else {
            valid_qualifiers
        };
        walk_content_dir(&dir, *kind, module_root, &mut sources, qualifiers)?;
    }

    Ok(sources)
}

/// Walk a content directory (agents/, rules/, or skills/), collecting .md files.
///
/// For rules and agents, subdirectories matching valid qualifier names are
/// walked for qualifier-only files. Base filenames are collected first, then
/// qualifier directories are scanned for files that have no base counterpart.
fn walk_content_dir(
    dir: &Path,
    kind: commands::provider::ContentKind,
    module_root: &Path,
    sources: &mut Vec<SourceFile>,
    valid_qualifiers: &HashSet<String>,
) -> Result<(), Error> {
    let entries = fs::read_dir(dir)
        .map_err(|e| Error::new(ErrorKind::Io, format!("cannot read {}: {e}", dir.display())))?;

    let mut base_filenames: HashSet<String> = HashSet::new();
    let mut qualifier_directories: Vec<(String, std::path::PathBuf)> = Vec::new();

    for entry in entries {
        let entry =
            entry.map_err(|e| Error::new(ErrorKind::Io, format!("directory entry error: {e}")))?;

        let path = entry.path();

        if path.is_dir() {
            if kind == commands::provider::ContentKind::Skills {
                walk_skill_dir(&path, kind, sources)?;
                continue;
            }

            let dirname = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            if dirname == "user" {
                continue;
            }

            if valid_qualifiers.contains(&dirname) {
                qualifier_directories.push((dirname, path));
            } else {
                walk_content_dir(&path, kind, module_root, sources, valid_qualifiers)?;
            }
            continue;
        }

        if path.extension().unwrap_or_default() != "md" {
            continue;
        }

        let filename = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        base_filenames.insert(filename);

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
            kind,
            passthrough: false,
            qualifier: None,
        });
    }

    for (qualifier_name, qualifier_path) in qualifier_directories {
        walk_qualifier_dir(
            &qualifier_path,
            &qualifier_name,
            kind,
            module_root,
            sources,
            &base_filenames,
        )?;
    }

    Ok(())
}

/// Walk a qualifier subdirectory, collecting only qualifier-only files.
///
/// Files that share a name with a base file are variant overrides, handled
/// by `variants::resolve` during assembly of the base file. Only files
/// with no base counterpart are collected as qualifier-only sources.
fn walk_qualifier_dir(
    dir: &Path,
    qualifier_name: &str,
    kind: commands::provider::ContentKind,
    module_root: &Path,
    sources: &mut Vec<SourceFile>,
    base_filenames: &HashSet<String>,
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
        if base_filenames.contains(&filename) {
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
            kind,
            passthrough: false,
            qualifier: Some(qualifier_name.to_string()),
        });
    }

    Ok(())
}

/// Walk a skill subdirectory. SKILL.md is assembled; other .md files are passthrough.
/// Subdirectories (user/, model qualifiers) are flattened into the skill root.
/// user/ files override root files with the same name.
///
/// Given `skills/Explain/`:
///
/// ```text
/// skills/Explain/SKILL.md            → passthrough: false (assembled)
/// skills/Explain/examples.md         → passthrough: true  (copied verbatim)
/// skills/Explain/user/Extra.md       → passthrough: true  (flattened to Explain/Extra.md)
/// skills/Explain/user/examples.md    → passthrough: true  (overrides root examples.md)
/// ```
fn walk_skill_dir(
    dir: &Path,
    kind: commands::provider::ContentKind,
    sources: &mut Vec<SourceFile>,
) -> Result<(), Error> {
    let skill_name = dir
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let mut file_map: std::collections::HashMap<String, SourceFile> =
        std::collections::HashMap::new();

    collect_skill_files(dir, &skill_name, kind, &mut file_map, false)?;

    let user_dir = dir.join("user");
    if user_dir.is_dir() {
        collect_skill_files(&user_dir, &skill_name, kind, &mut file_map, true)?;
    }

    sources.extend(file_map.into_values());
    Ok(())
}

fn collect_skill_files(
    dir: &Path,
    skill_name: &str,
    kind: commands::provider::ContentKind,
    file_map: &mut std::collections::HashMap<String, SourceFile>,
    is_overlay: bool,
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
        let flattened_relative = format!("skills/{skill_name}/{filename}");
        let content = read_file(&path)?;

        if is_overlay && file_map.contains_key(&filename) {
            eprintln!("  override  skills/{skill_name}/user/{filename} → {filename}");
        } else if is_overlay {
            eprintln!("  flatten   skills/{skill_name}/user/{filename} → {filename}");
        }

        file_map.insert(
            filename,
            SourceFile {
                relative_path: flattened_relative,
                full_path: path.to_string_lossy().to_string(),
                content,
                kind,
                passthrough: !is_skill_file,
                qualifier: None,
            },
        );
    }

    Ok(())
}

/// Build the set of valid qualifier names from provider names and model IDs.
///
/// A qualifier is valid if it matches either a provider name (e.g., "claude")
/// or a segment of any model ID (e.g., "sonnet" from "claude-sonnet-4-6").
pub fn build_valid_qualifiers(
    provider_names: &[String],
    models: &std::collections::HashMap<String, Vec<String>>,
) -> HashSet<String> {
    let mut qualifiers = HashSet::new();
    for name in provider_names {
        qualifiers.insert(name.clone());
    }
    for model_ids in models.values() {
        for model_id in model_ids {
            for segment in model_id.split('-') {
                if !segment.is_empty() {
                    qualifiers.insert(segment.to_string());
                }
            }
        }
    }
    qualifiers
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    macro_rules! fixture {
        ($name:expr) => {
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/input/",
                $name
            ))
        };
    }

    const BASE_RULE: &str = fixture!("qualifier-base-rule.md");
    const QUALIFIER_ONLY: &str = fixture!("qualifier-only-rule.md");
    const VARIANT_OVERRIDE: &str = fixture!("qualifier-variant-override.md");

    fn make_models() -> HashMap<String, Vec<String>> {
        let mut models = HashMap::new();
        models.insert(
            "claude".to_string(),
            vec![
                "claude-opus-4-6".to_string(),
                "claude-sonnet-4-6".to_string(),
            ],
        );
        models.insert("codex".to_string(), vec!["o4-mini".to_string()]);
        models.insert(
            "opencode".to_string(),
            vec!["claude-sonnet-4-6".to_string()],
        );
        models
    }

    fn scaffold_kind(root: &std::path::Path, kind: &str) -> std::path::PathBuf {
        let dir = root.join(kind);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn build_qualifiers_includes_provider_names() {
        let providers = vec!["claude".to_string(), "codex".to_string()];
        let qualifiers = build_valid_qualifiers(&providers, &make_models());
        assert!(qualifiers.contains("claude"));
        assert!(qualifiers.contains("codex"));
    }

    #[test]
    fn build_qualifiers_includes_model_tier_segments() {
        let providers = vec!["claude".to_string()];
        let qualifiers = build_valid_qualifiers(&providers, &make_models());
        assert!(qualifiers.contains("sonnet"));
        assert!(qualifiers.contains("opus"));
    }

    #[test]
    fn qualifier_only_files_have_qualifier_set() {
        let dir = tempfile::tempdir().unwrap();
        let rules = scaffold_kind(dir.path(), "rules");
        std::fs::write(rules.join("BaseRule.md"), BASE_RULE).unwrap();
        let sonnet = rules.join("sonnet");
        std::fs::create_dir(&sonnet).unwrap();
        std::fs::write(sonnet.join("QualifierOnly.md"), QUALIFIER_ONLY).unwrap();

        let valid = HashSet::from(["sonnet".to_string()]);
        let sources = collect(dir.path(), &valid).unwrap();

        let base = sources
            .iter()
            .find(|s| s.relative_path.contains("BaseRule"))
            .unwrap();
        assert!(base.qualifier.is_none());
        let qualified = sources
            .iter()
            .find(|s| s.relative_path.contains("QualifierOnly"))
            .unwrap();
        assert_eq!(qualified.qualifier, Some("sonnet".to_string()));
    }

    #[test]
    fn skips_variant_overrides_in_qualifier_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let rules = scaffold_kind(dir.path(), "rules");
        std::fs::write(rules.join("BaseRule.md"), BASE_RULE).unwrap();
        let codex = rules.join("codex");
        std::fs::create_dir(&codex).unwrap();
        std::fs::write(codex.join("BaseRule.md"), VARIANT_OVERRIDE).unwrap();

        let valid = HashSet::from(["codex".to_string()]);
        let sources = collect(dir.path(), &valid).unwrap();

        let matching: Vec<_> = sources
            .iter()
            .filter(|s| s.relative_path.contains("BaseRule"))
            .collect();
        assert_eq!(matching.len(), 1);
        assert!(matching[0].qualifier.is_none());
    }

    #[test]
    fn skips_user_directory() {
        let dir = tempfile::tempdir().unwrap();
        let rules = scaffold_kind(dir.path(), "rules");
        std::fs::write(rules.join("BaseRule.md"), BASE_RULE).unwrap();
        let user = rules.join("user");
        std::fs::create_dir(&user).unwrap();
        std::fs::write(user.join("UserOnly.md"), QUALIFIER_ONLY).unwrap();

        let valid = HashSet::from(["user".to_string()]);
        let sources = collect(dir.path(), &valid).unwrap();
        assert!(
            sources
                .iter()
                .all(|s| !s.relative_path.contains("UserOnly"))
        );
    }

    #[test]
    fn walks_non_qualifier_subdirectories_as_content() {
        let dir = tempfile::tempdir().unwrap();
        let rules = scaffold_kind(dir.path(), "rules");
        std::fs::write(rules.join("BaseRule.md"), BASE_RULE).unwrap();
        let subdir = rules.join("category");
        std::fs::create_dir(&subdir).unwrap();
        std::fs::write(subdir.join("SubRule.md"), QUALIFIER_ONLY).unwrap();

        let valid = HashSet::from(["sonnet".to_string()]);
        let sources = collect(dir.path(), &valid).unwrap();
        assert!(
            sources.iter().any(
                |source| source.relative_path.contains("SubRule") && source.qualifier.is_none()
            )
        );
    }

    #[test]
    fn skills_ignore_qualifier_directories() {
        let dir = tempfile::tempdir().unwrap();
        let skills = scaffold_kind(dir.path(), "skills");
        let sonnet_skill = skills.join("sonnet");
        std::fs::create_dir(&sonnet_skill).unwrap();
        std::fs::write(sonnet_skill.join("SKILL.md"), QUALIFIER_ONLY).unwrap();

        let valid = HashSet::from(["sonnet".to_string()]);
        let sources = collect(dir.path(), &valid).unwrap();

        let skill = sources
            .iter()
            .find(|s| s.relative_path.contains("SKILL"))
            .unwrap();
        assert!(skill.qualifier.is_none());
        assert_eq!(skill.kind, commands::provider::ContentKind::Skills);
    }

    #[test]
    fn agents_qualifier_directory() {
        let dir = tempfile::tempdir().unwrap();
        let agents = scaffold_kind(dir.path(), "agents");
        std::fs::write(agents.join("BaseAgent.md"), BASE_RULE).unwrap();
        let codex = agents.join("codex");
        std::fs::create_dir(&codex).unwrap();
        std::fs::write(codex.join("CodexOnly.md"), QUALIFIER_ONLY).unwrap();

        let valid = HashSet::from(["codex".to_string()]);
        let sources = collect(dir.path(), &valid).unwrap();

        let codex_only = sources
            .iter()
            .find(|s| s.relative_path.contains("CodexOnly"))
            .unwrap();
        assert_eq!(codex_only.qualifier, Some("codex".to_string()));
        assert_eq!(codex_only.kind, commands::provider::ContentKind::Agents);
    }

    #[test]
    fn walk_skill_dir_flattens_user_subdirectory() {
        let dir = tempfile::tempdir().unwrap();
        let skill_dir = scaffold_kind(dir.path(), "skills").join("TestSkill");
        let user_dir = skill_dir.join("user");
        std::fs::create_dir_all(&user_dir).unwrap();

        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: TestSkill\n---\n# TestSkill",
        )
        .unwrap();
        std::fs::write(skill_dir.join("Reference.md"), "Root reference content").unwrap();
        std::fs::write(user_dir.join("Extra.md"), "User-only companion").unwrap();
        std::fs::write(user_dir.join("Reference.md"), "User override content").unwrap();

        let mut sources = Vec::new();
        walk_skill_dir(
            &skill_dir,
            commands::provider::ContentKind::Skills,
            &mut sources,
        )
        .unwrap();

        assert_eq!(
            sources.len(),
            3,
            "expected 3 sources, got {}",
            sources.len()
        );

        let skill = sources
            .iter()
            .find(|s| s.relative_path.contains("SKILL.md"))
            .unwrap();
        assert!(!skill.passthrough, "SKILL.md should not be passthrough");

        let reference = sources
            .iter()
            .find(|s| s.relative_path.contains("Reference.md"))
            .unwrap();
        assert!(reference.passthrough, "Reference.md should be passthrough");
        assert_eq!(
            reference.content, "User override content",
            "user/ should override root"
        );

        let extra = sources
            .iter()
            .find(|s| s.relative_path.contains("Extra.md"))
            .unwrap();
        assert!(extra.passthrough, "Extra.md should be passthrough");
        assert_eq!(extra.content, "User-only companion");

        for source in &sources {
            assert!(
                !source.relative_path.contains("user/"),
                "relative path should be flattened: {}",
                source.relative_path
            );
        }
    }
}
