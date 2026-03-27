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
    let models = config::load_models(module_root);
    let provider_names: Vec<String> = providers.keys().cloned().collect();
    let valid_qualifiers = sources::build_valid_qualifiers(&provider_names, &models);
    let source_files = sources::collect(module_root, &valid_qualifiers)?;

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
        let has_strip_links =
            assembly_rules.contains(&commands::provider::AssemblyRule::StripLinks);

        for source in &source_files {
            if source.qualifier.as_ref().is_some_and(|qualifier| {
                !qualifier_matches_provider(qualifier, provider_name, &models)
            }) {
                continue;
            }
            let kind_keep_fields = provider_config
                .keep_fields
                .as_ref()
                .and_then(|fields_by_kind| fields_by_kind.get(&source.kind))
                .cloned()
                .unwrap_or_default();

            let model_tiers = provider_config
                .models
                .clone()
                .unwrap_or_default();

            let assembled = pipeline::assemble_source(
                source,
                module_root,
                provider_name,
                &kind_keep_fields,
                &tool_mappings,
                &model_tiers,
                has_strip_links,
            )?;

            // For skills, preserve the skill directory: skills/SceneReview/SKILL.md
            // For agents/rules, use just the filename: agents/GameMaster.md
            // For qualifier-only files, strip the qualifier directory too:
            //   rules/sonnet/ReviewDiscipline.md → ReviewDiscipline.md
            let stripped_kind = source
                .relative_path
                .strip_prefix(&format!("{}/", source.kind))
                .unwrap_or(&source.relative_path);
            let relative_within_kind = if source.qualifier.is_some() {
                stripped_kind
                    .split_once('/')
                    .map_or(stripped_kind, |(_, filename)| filename)
            } else {
                stripped_kind
            };

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

/// Check whether a qualifier directory matches a given provider.
///
/// A qualifier matches if it is either the provider name itself, or if
/// any model ID for that provider contains the qualifier as a substring.
fn qualifier_matches_provider(
    qualifier: &str,
    provider_name: &str,
    models: &std::collections::HashMap<String, Vec<String>>,
) -> bool {
    if qualifier == provider_name {
        return true;
    }
    if let Some(model_ids) = models.get(provider_name) {
        return model_ids.iter().any(|id| id.contains(qualifier));
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

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

    #[test]
    fn direct_provider_name_matches() {
        let models = make_models();
        assert!(qualifier_matches_provider("claude", "claude", &models));
        assert!(qualifier_matches_provider("codex", "codex", &models));
    }

    #[test]
    fn model_tier_matches_provider_with_that_model() {
        let models = make_models();
        assert!(qualifier_matches_provider("sonnet", "claude", &models));
        assert!(qualifier_matches_provider("opus", "claude", &models));
    }

    #[test]
    fn model_tier_matches_across_providers() {
        let models = make_models();
        assert!(qualifier_matches_provider("sonnet", "opencode", &models));
    }

    #[test]
    fn model_tier_does_not_match_unrelated_provider() {
        let models = make_models();
        assert!(!qualifier_matches_provider("sonnet", "codex", &models));
        assert!(!qualifier_matches_provider("opus", "codex", &models));
    }

    #[test]
    fn unknown_qualifier_does_not_match() {
        let models = make_models();
        assert!(!qualifier_matches_provider("gpt5", "claude", &models));
    }

    #[test]
    fn provider_not_in_models_only_matches_by_name() {
        let models = make_models();
        assert!(qualifier_matches_provider("gemini", "gemini", &models));
        assert!(!qualifier_matches_provider("sonnet", "gemini", &models));
    }
}
