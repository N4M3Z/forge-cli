use commands::error::{Error, ErrorKind};
use commands::provider;
use commands::yaml;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Embedded at compile time so the binary works when symlinked away from
/// its source tree (e.g. ~/.local/bin/forge).
const EMBEDDED_DEFAULTS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/defaults.yaml"));
const EMBEDDED_REMAP_TOOLS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/config/remap-tools.yaml"
));
const EMBEDDED_MODELS: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/config/models.yaml"));

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
    // defaults.yaml is optional; if missing, we fall back to embedded defaults
    // to allow modules without local configuration to install.
    let defaults_content = if defaults_path.is_file() {
        read_file(&defaults_path)?
    } else {
        String::new()
    };

    let config_path = module_root.join("config.yaml");
    if config_path.is_file() {
        let config_content = read_file(&config_path)?;
        yaml::deep_merge(&defaults_content, &config_content)
            .map_err(|e| Error::new(ErrorKind::Config, format!("config merge failed: {e}")))
    } else {
        Ok(defaults_content)
    }
}

/// Load provider configurations, merging module config with embedded defaults.
///
/// The embedded defaults provide the base (`target`, assembly rules, `keep_fields`,
/// `models`). The module's `providers:` section overrides specific fields per provider.
/// If the module has no providers: section, embedded defaults are used entirely.
pub fn load_providers(config: &str) -> Result<HashMap<String, provider::ProviderConfig>, Error> {
    let embedded_providers = provider::load_providers(EMBEDDED_DEFAULTS).map_err(|error| {
        Error::new(
            ErrorKind::Config,
            format!("failed to load embedded provider config: {error}"),
        )
    })?;

    let Ok(module_config) = yaml::deep_merge(EMBEDDED_DEFAULTS, config) else {
        return Ok(embedded_providers);
    };

    match provider::load_providers(&module_config) {
        Ok(providers) => Ok(providers),
        Err(error) => {
            eprintln!(
                "warning: module config incompatible with provider schema ({error}), using embedded defaults"
            );
            Ok(embedded_providers)
        }
    }
}

/// Load the allowlist of settings filenames the dashboard surfaces per harness,
/// from `dashboard.settings_files` in the merged config. Falls back to the
/// embedded defaults when the module config omits the section.
#[cfg_attr(not(feature = "dashboard"), allow(dead_code))]
pub fn load_settings_filenames(module_root: &Path) -> Vec<String> {
    let merged = load_merged_config(module_root).unwrap_or_default();
    let from_module = parse_settings_filenames(&merged);
    if from_module.is_empty() {
        parse_settings_filenames(EMBEDDED_DEFAULTS)
    } else {
        from_module
    }
}

#[cfg_attr(not(feature = "dashboard"), allow(dead_code))]
fn parse_settings_filenames(config: &str) -> Vec<String> {
    #[derive(serde::Deserialize, Default)]
    struct DashboardConfig {
        #[serde(default)]
        settings_files: Vec<String>,
    }
    #[derive(serde::Deserialize, Default)]
    struct Wrapper {
        #[serde(default)]
        dashboard: DashboardConfig,
    }
    match serde_yaml::from_str::<Wrapper>(config) {
        Ok(wrapper) => wrapper.dashboard.settings_files,
        Err(error) => {
            eprintln!(
                "warning: failed to parse dashboard.settings_files ({error}), using embedded defaults"
            );
            Vec::new()
        }
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

/// Load model definitions from models.yaml, falling back to embedded defaults.
///
/// Returns an empty map if neither the module file nor the embedded file
/// can be parsed (all model-tier qualifiers become unresolvable).
pub fn load_models(module_root: &Path) -> HashMap<String, Vec<String>> {
    let models_path = module_root.join("config/models.yaml");
    let content = if models_path.is_file() {
        read_file(&models_path).ok()
    } else {
        Some(EMBEDDED_MODELS.to_string())
    };

    match content {
        Some(yaml) => match provider::load_models(&yaml) {
            Ok(models) => models,
            Err(error) => {
                eprintln!(
                    "warning: failed to parse models config ({error}), using embedded defaults"
                );
                provider::load_models(EMBEDDED_MODELS).unwrap_or_default()
            }
        },
        None => HashMap::new(),
    }
}

/// Load the source URI for provenance from module.yaml.
///
/// Checks for a `repository` field first (full URL), then falls back
/// to the `name` field as a plain identifier.
pub fn load_source_uri(module_root: &Path) -> String {
    match commands::module::load(module_root) {
        Ok(manifest) => manifest.source_uri().to_string(),
        Err(_) => String::new(),
    }
}

#[cfg(test)]
mod tests;
