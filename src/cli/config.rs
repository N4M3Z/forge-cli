use commands::error::{Error, ErrorKind};
use commands::provider;
use commands::yaml;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Embedded at compile time so the binary works when symlinked away from
/// its source tree (e.g. ~/.local/bin/forge).
const EMBEDDED_DEFAULTS: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/defaults.yaml"));
const EMBEDDED_REMAP_TOOLS: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/config/remap-tools.yaml"));

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

/// Load provider configurations from merged config YAML.
/// Falls back to embedded defaults when the module lacks a providers: section.
pub fn load_providers(config: &str) -> Result<HashMap<String, provider::ProviderConfig>, Error> {
    if let Ok(providers) = provider::load_providers(config) {
        Ok(providers)
    } else {
        provider::load_providers(EMBEDDED_DEFAULTS).map_err(|e| {
            Error::new(
                ErrorKind::Config,
                format!("failed to load embedded provider config: {e}"),
            )
        })
    }
}

/// Load remap-tools.yaml from the module, falling back to embedded defaults.
pub fn load_remap_tools(module_root: &Path) -> Result<Option<String>, Error> {
    let module_remap = module_root.join("config/remap-tools.yaml");
    if module_remap.is_file() {
        return Ok(Some(read_file(&module_remap)?));
    }
    Ok(Some(EMBEDDED_REMAP_TOOLS.to_string()))
}

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
