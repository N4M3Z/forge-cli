use commands::error::{Error, ErrorKind};
use commands::provider;
use commands::yaml;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Read a file to string with consistent error handling.
pub fn read_file(path: &Path) -> Result<String, Error> {
    fs::read_to_string(path).map_err(|e| {
        Error::new(
            ErrorKind::Io,
            format!("cannot read {}: {e}", path.display()),
        )
    })
}

/// Read and deep-merge defaults.yaml with optional config.yaml.
///
/// ```text
/// defaults.yaml (committed)  +  config.yaml (gitignored)
///   → deep_merge(defaults, config)
///   → merged YAML string
/// ```
pub fn load_merged_config(module_root: &Path) -> Result<String, Error> {
    let defaults_path = module_root.join("defaults.yaml");
    let defaults_content = read_file(&defaults_path)?;

    let config_path = module_root.join("config.yaml");
    if config_path.is_file() {
        let config_content = read_file(&config_path)?;
        yaml::deep_merge(&defaults_content, &config_content)
            .map_err(|e| Error::new(ErrorKind::Config, format!("config merge failed: {e}")))
    } else {
        Ok(defaults_content)
    }
}

/// Resolve the forge-cli installation directory (where the forge binary lives).
///
/// Used to locate forge-cli's own defaults.yaml and config/ directory
/// as fallback when the module doesn't provide provider config.
fn forge_cli_root() -> Option<std::path::PathBuf> {
    let exe_path = std::env::current_exe().ok()?;
    // Binary is at <root>/target/debug/forge or <root>/bin/forge
    // Walk up to find defaults.yaml
    let mut dir = exe_path.parent()?;
    for _ in 0..5 {
        if dir.join("defaults.yaml").is_file() {
            return Some(dir.to_path_buf());
        }
        dir = dir.parent()?;
    }
    None
}

/// Load provider configurations from merged config YAML.
///
/// If the module's config doesn't have a `providers:` section with
/// forge-cli's schema (target/assembly), falls back to forge-cli's
/// own defaults.yaml for provider deployment config.
pub fn load_providers(config: &str) -> Result<HashMap<String, provider::ProviderConfig>, Error> {
    if let Ok(providers) = provider::load_providers(config) { Ok(providers) } else {
        // Module doesn't have provider deployment config — use forge-cli's own
        let cli_root = forge_cli_root().ok_or_else(|| {
            Error::new(ErrorKind::Config, "cannot locate forge-cli defaults.yaml")
        })?;
        let cli_defaults = read_file(&cli_root.join("defaults.yaml"))?;
        provider::load_providers(&cli_defaults).map_err(|e| {
            Error::new(
                ErrorKind::Config,
                format!("failed to load forge-cli provider config: {e}"),
            )
        })
    }
}



/// Load remap-tools.yaml, checking the module first then falling back to forge-cli's own.
pub fn load_remap_tools(module_root: &Path) -> Result<Option<String>, Error> {
    let module_remap = module_root.join("config/remap-tools.yaml");
    if module_remap.is_file() {
        return Ok(Some(read_file(&module_remap)?));
    }
    if let Some(cli_root) = forge_cli_root() {
        let cli_remap = cli_root.join("config/remap-tools.yaml");
        if cli_remap.is_file() {
            return Ok(Some(read_file(&cli_remap)?));
        }
    }
    Ok(None)
}

/// Load tool name mappings for a specific provider from remap content.
///
/// Returns an empty map when no remap content exists or the provider
/// has no entry.
pub fn load_tool_mappings(
    remap_content: Option<&String>,
    provider_name: &str,
) -> Result<HashMap<String, String>, Error> {
    match remap_content {
        Some(content) => provider::load_tool_mappings(content, provider_name).map_err(|e| {
            Error::new(
                ErrorKind::Config,
                format!("failed to load tool mappings: {e}"),
            )
        }),
        None => Ok(HashMap::new()),
    }
}

/// Parse `keep_fields` from merged config as a comma-separated list.
pub fn parse_keep_fields(config: &str) -> Vec<String> {
    let raw = yaml::yaml_list(config, "keep_fields").unwrap_or_default();
    if raw.is_empty() {
        Vec::new()
    } else {
        raw.split(", ").map(String::from).collect()
    }
}