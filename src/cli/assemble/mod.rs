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
    if !module_root.is_dir() {
        return Err(Error::new(
            commands::error::ErrorKind::Io,
            format!("module directory not found: {}", module_root.display()),
        ));
    }
    let mut result = ActionResult::new();

    let merged_config = config::load_merged_config(module_root)?;
    let providers = config::load_providers(&merged_config)?;
    let remap_content = config::load_remap_tools(module_root)?;
    let models = config::load_models(module_root);
    let source_uri = config::load_source_uri(module_root);
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

        let model_tiers = provider_config.models.clone().unwrap_or_default();

        for source in &source_files {
            if let Some(deployed) = assemble_source_for_provider(
                source,
                module_root,
                provider_name,
                provider_config,
                &provider_build_dir,
                &tool_mappings,
                &assembly_rules,
                &model_tiers,
                &models,
                &source_uri,
            )? {
                result.installed.push(deployed);
            }
        }
    }

    Ok(result)
}

#[allow(clippy::too_many_arguments)]
fn assemble_source_for_provider(
    source: &sources::SourceFile,
    module_root: &Path,
    provider_name: &str,
    provider_config: &commands::provider::ProviderConfig,
    provider_build_dir: &Path,
    tool_mappings: &std::collections::HashMap<String, String>,
    assembly_rules: &[commands::provider::AssemblyRule],
    model_tiers: &std::collections::HashMap<String, Vec<String>>,
    models: &std::collections::HashMap<String, Vec<String>>,
    source_uri: &str,
) -> Result<Option<DeployedFile>, Error> {
    if source
        .qualifier
        .as_ref()
        .is_some_and(|qualifier| !qualifier_matches_provider(qualifier, provider_name, models))
    {
        return Ok(None);
    }

    if source.targets.as_ref().is_some_and(|file_targets| {
        !file_targets
            .iter()
            .any(|target| provider_config.matches_target(target, provider_name))
    }) {
        return Ok(None);
    }

    let kind_keep_fields = provider_config
        .keep_fields
        .as_ref()
        .and_then(|fields_by_kind| fields_by_kind.get(source.kind.as_str()))
        .cloned()
        .unwrap_or_default();

    let mut assembled = pipeline::assemble_source(
        source,
        module_root,
        provider_name,
        &kind_keep_fields,
        model_tiers,
        assembly_rules.contains(&commands::provider::AssemblyRule::StripLinks),
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

    // Apply transformation rules (kebab-case, kebab-case-agents, remap-tools, etc.)
    let (transformed_content, transformed_filename) = commands::transform::apply_rules(
        &assembled,
        relative_within_kind,
        assembly_rules,
        tool_mappings,
        source.kind.as_str(),
    )
    .map_err(|e| commands::error::Error::new(commands::error::ErrorKind::Validate, e))?;

    assembled = transformed_content;

    // Always ensure a trailing newline for POSIX text file convention
    // before calculating the hash for provenance and writing to disk.
    if !assembled.is_empty() && !assembled.ends_with('\n') {
        assembled.push('\n');
    }

    let output_path = provider_build_dir
        .join(source.kind.as_str())
        .join(&transformed_filename);
    let manifest_key = format!("{}/{}/{}", provider_name, source.kind, transformed_filename);

    output::write_file(&output_path, &assembled)?;

    let statement = provenance::build_statement(&manifest_key, &assembled, source, source_uri);
    provenance::write_sidecar(&output_path, &statement)?;

    Ok(Some(DeployedFile {
        source: source.relative_path.clone(),
        target: output_path.to_string_lossy().to_string(),
        provider: provider_name.to_string(),
    }))
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

/// Apply kebab-case transformation to each segment of a path.
#[cfg(test)]
fn apply_kebab_case_to_path(path: &str) -> String {
    path.split('/')
        .map(commands::transform::to_kebab_case)
        .collect::<Vec<String>>()
        .join("/")
}

#[cfg(test)]
mod tests;
