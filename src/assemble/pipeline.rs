use std::collections::HashMap;

use crate::manifest;
use crate::provider::{AssemblyRule, ProviderConfig};
use crate::transform;

/// A source file to be assembled, with optional variant overlay.
pub struct SourceFile<'a> {
    /// Path relative to the module root (e.g. `rules/MyRule.md`).
    pub relative_path: &'a str,
    /// Raw content of the source file.
    pub content: &'a str,
    /// Optional variant content to merge before assembly.
    pub variant_content: Option<&'a str>,
    /// When true, skip content transforms — only apply filename transforms.
    pub passthrough: bool,
}

/// Result of assembling a single source file for one provider.
pub struct AssembledFile {
    /// Original source path (relative to module root).
    pub source_path: String,
    /// Output path: `{provider}/{content_kind}/{filename}`.
    pub output_path: String,
    /// Assembled content (after transforms and rule application).
    pub content: String,
    /// SHA-256 digests of each input that contributed to this output.
    pub source_hashes: Vec<(String, String)>,
}

/// Assemble a single source file for one provider.
///
/// Pipeline steps:
///   1. If `passthrough`, use raw content; otherwise run full assembly
///   2. Apply provider-specific rules (kebab-case, remap-tools, etc.)
///   3. Compute input digests (source + optional variant)
///   4. Build output path from provider name and content kind
pub fn assemble_file(
    source: &SourceFile,
    provider_name: &str,
    rules: &[AssemblyRule],
    keep_fields: &[&str],
    tool_mappings: &HashMap<String, String, impl std::hash::BuildHasher>,
) -> Result<AssembledFile, String> {
    let filename = extract_filename(source.relative_path);
    let content_kind = extract_content_kind(source.relative_path);

    // Step 1: content assembly
    let assembled = if source.passthrough {
        source.content.to_string()
    } else {
        let has_strip_links = rules.contains(&AssemblyRule::StripLinks);
        super::assemble(source.content, source.variant_content, keep_fields, has_strip_links)
    };

    // Step 2: apply provider rules
    let (content, transformed_filename) =
        transform::apply_rules(&assembled, filename, rules, tool_mappings)?;

    // Step 3: input digests
    let mut source_hashes = vec![(
        source.relative_path.to_string(),
        manifest::content_sha256(source.content),
    )];
    if let Some(vc) = source.variant_content {
        source_hashes.push((
            format!("{} (variant)", source.relative_path),
            manifest::content_sha256(vc),
        ));
    }

    // Step 4: output path
    let output_path = format!("{provider_name}/{content_kind}/{transformed_filename}");

    Ok(AssembledFile {
        source_path: source.relative_path.to_string(),
        output_path,
        content,
        source_hashes,
    })
}

/// Assemble all source files across all providers.
///
/// For each provider, parses assembly rule names into `AssemblyRule` variants,
/// then assembles every source file. Errors are collected, not fatal —
/// partial success is expected when some rules are unknown or files malformed.
pub fn assemble_module(
    sources: &[SourceFile],
    providers: &HashMap<String, ProviderConfig>,
    tool_mappings_by_provider: &HashMap<String, HashMap<String, String>>,
    keep_fields: &[&str],
) -> (Vec<AssembledFile>, Vec<String>) {
    let mut results = Vec::new();
    let mut errors = Vec::new();

    for (provider_name, config) in providers {
        // Parse rule names into AssemblyRule variants
        let rule_names = config.assembly.as_deref().unwrap_or(&[]);
        let mut parsed_rules = Vec::with_capacity(rule_names.len());
        for name in rule_names {
            match AssemblyRule::from_name(name) {
                Ok(rule) => parsed_rules.push(rule),
                Err(err) => errors.push(format!("{provider_name}: {err}")),
            }
        }

        let empty_mappings = HashMap::default();
        let tool_mappings = tool_mappings_by_provider
            .get(provider_name)
            .unwrap_or(&empty_mappings);

        for source in sources {
            match assemble_file(
                source,
                provider_name,
                &parsed_rules,
                keep_fields,
                tool_mappings,
            ) {
                Ok(assembled) => results.push(assembled),
                Err(err) => errors.push(format!("{provider_name}/{}: {err}", source.relative_path)),
            }
        }
    }

    (results, errors)
}

/// Extract the filename from a relative path.
fn extract_filename(relative_path: &str) -> &str {
    relative_path
        .rsplit_once('/')
        .map_or(relative_path, |(_, name)| name)
}

/// Extract the content kind (first path segment) from a relative path.
///
/// For `rules/MyRule.md`, returns `rules`.
/// For a bare filename like `README.md`, returns an empty string.
fn extract_content_kind(relative_path: &str) -> &str {
    relative_path.split_once('/').map_or("", |(kind, _)| kind)
}
