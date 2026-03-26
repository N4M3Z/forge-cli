mod output;
mod pipeline;
mod provenance;
pub mod sources;

use commands::error::Error;
use commands::result::{ActionResult, DeployedFile};
use std::path::Path;

use crate::cli::config;

/// Assemble module content into the build/ directory.
///
/// Given a module at `path`:
///
/// ```text
/// module/
///   defaults.yaml
///   config.yaml                  (optional override)
///   config/remap-tools.yaml      (optional tool mappings)
///   agents/SecurityArchitect.md
///   rules/MyRule.md
///   skills/Explain/SKILL.md
/// ```
///
/// Produces:
///
/// ```text
/// module/build/
///   claude/agents/SecurityArchitect.md
///   claude/agents/SecurityArchitect.yaml
///   claude/rules/MyRule.md
///   claude/rules/MyRule.yaml
///   gemini/agents/security-architect.md  (with remapped tools)
///   gemini/agents/security-architect.yaml
/// ```
pub fn execute(path: &str) -> Result<ActionResult, Error> {
    let module_root = Path::new(path);
    let mut result = ActionResult::new();

    let merged_config = config::load_merged_config(module_root)?;
    let providers = config::load_providers(&merged_config)?;
    let remap_content = config::load_remap_tools(module_root)?;
    let keep_fields = config::parse_keep_fields(&merged_config);
    let source_files = sources::collect(module_root)?;

    let build_dir = module_root.join("build");

    // Assembly always starts clean — no stale files from previous runs
    if build_dir.is_dir() {
        std::fs::remove_dir_all(&build_dir).map_err(|e| {
            commands::error::Error::new(
                commands::error::ErrorKind::Io,
                format!("cannot clean build directory: {e}"),
            )
        })?;
    }

    for (provider_name, provider_config) in &providers {
        let provider_build_dir = build_dir.join(provider_name);
        let tool_mappings = config::load_tool_mappings(remap_content.as_ref(), provider_name)?;

        // Parse assembly rules for this provider
        let assembly_rules: Vec<commands::provider::AssemblyRule> = provider_config
            .assembly
            .as_ref()
            .unwrap_or(&Vec::new())
            .iter()
            .filter_map(|name| commands::provider::AssemblyRule::from_name(name).ok())
            .collect();

        let has_kebab_case = assembly_rules.contains(&commands::provider::AssemblyRule::KebabCase);

        for source in &source_files {
            let assembled = pipeline::assemble_source(
                source,
                module_root,
                provider_name,
                &keep_fields,
                &tool_mappings,
            )?;

            // For skills, preserve the skill directory: skills/SceneReview/SKILL.md
            // For agents/rules, use just the filename: agents/GameMaster.md
            let relative_within_kind = source
                .relative_path
                .strip_prefix(&format!("{}/", source.kind))
                .unwrap_or(&source.relative_path);

            // Apply filename transforms (kebab-case for gemini/opencode)
            let transformed_path = if has_kebab_case {
                apply_kebab_case_to_path(relative_within_kind)
            } else {
                relative_within_kind.to_string()
            };

            let output_path = provider_build_dir
                .join(&source.kind)
                .join(&transformed_path);
            let manifest_key = format!("{}/{}/{}", provider_name, source.kind, transformed_path);

            output::write_file(&output_path, &assembled)?;

            let statement = provenance::build_statement(&manifest_key, &assembled, source);
            provenance::write_sidecar(&output_path, &statement)?;

            result.installed.push(DeployedFile {
                source: source.relative_path.clone(),
                target: output_path.to_string_lossy().to_string(),
                provider: provider_name.clone(),
            });
        }
    }

    Ok(result)
}

/// Apply kebab-case conversion to path segments (directory names and filenames).
///
/// "SceneReview/SKILL.md" → "scene-review/SKILL.md"
/// "GameMaster.md" → "game-master.md"
fn apply_kebab_case_to_path(path: &str) -> String {
    let parts: Vec<&str> = path.split('/').collect();
    let mut result = Vec::new();

    for part in &parts {
        if let Some((stem, ext)) = part.rsplit_once('.') {
            let kebab_stem = commands::transform::to_kebab_case(stem);
            result.push(format!("{kebab_stem}.{ext}"));
        } else {
            result.push(commands::transform::to_kebab_case(part));
        }
    }

    result.join("/")
}
