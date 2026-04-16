use serde::Deserialize;
use std::collections::HashMap;

// --- Content Kind ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContentKind {
    Agents,
    Skills,
    Rules,
}

impl ContentKind {
    pub const ALL: &[ContentKind] = &[Self::Agents, Self::Skills, Self::Rules];

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Agents => "agents",
            Self::Skills => "skills",
            Self::Rules => "rules",
        }
    }
}

impl std::fmt::Display for ContentKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

// --- Types ---

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssemblyRule {
    KebabCase,
    KebabCaseAgents,
    RemapTools,
    AgentsToToml,
    StripLinks,
}

impl AssemblyRule {
    pub fn from_name(name: &str) -> Result<Self, String> {
        match name {
            "kebab-case" => Ok(Self::KebabCase),
            "kebab-case-agents" => Ok(Self::KebabCaseAgents),
            "remap-tools" => Ok(Self::RemapTools),
            "agents-to-toml" => Ok(Self::AgentsToToml),
            "strip-links" => Ok(Self::StripLinks),
            other => Err(format!("unknown assembly rule: '{other}'")),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ProviderConfig {
    pub target: String,
    pub assembly: Option<Vec<String>>,
    pub deploy: Option<Vec<String>>,
    pub keep_fields: Option<HashMap<String, Vec<String>>>,
    pub models: Option<HashMap<String, Vec<String>>>,
    pub aliases: Option<Vec<String>>,
}

impl ProviderConfig {
    pub fn matches_target(&self, target_name: &str, provider_key: &str) -> bool {
        if target_name == provider_key {
            return true;
        }

        if target_name == self.target || target_name == self.target.trim_start_matches('.') {
            return true;
        }

        self.aliases
            .as_ref()
            .is_some_and(|aliases| aliases.iter().any(|alias| alias == target_name))
    }
}

// --- Loading ---

#[derive(Deserialize)]
struct Wrapper {
    providers: HashMap<String, ProviderConfig>,
}

pub fn load_providers(defaults_content: &str) -> Result<HashMap<String, ProviderConfig>, String> {
    let wrapper: Wrapper = parse_yaml(defaults_content, "providers")?;
    Ok(wrapper.providers)
}

pub fn load_models(models_content: &str) -> Result<HashMap<String, Vec<String>>, String> {
    parse_yaml(models_content, "models")
}

/// Load tool name mappings for a specific provider from remap-tools YAML.
///
/// The YAML file is structured as:
///
/// ```yaml
/// gemini:
///     Read: read_file
///     Write: write_file
/// ```
///
/// Returns the mapping for the given provider, or an empty map if the
/// provider has no entry.
pub fn load_tool_mappings(
    remap_content: &str,
    provider_name: &str,
) -> Result<HashMap<String, String>, String> {
    let parsed: HashMap<String, HashMap<String, String>> =
        parse_yaml(remap_content, "remap-tools")?;

    match parsed.get(provider_name) {
        Some(mappings) => Ok(mappings.clone()),
        None => Ok(HashMap::new()),
    }
}

// --- Lookup ---

pub fn map_tool(tool: &str, mappings: &HashMap<String, String>) -> String {
    if let Some(mapped) = mappings.get(tool) {
        return mapped.clone();
    }
    tool.to_string()
}

// --- Validation ---

pub fn validate_qualifier(
    qualifier_name: &str,
    models: &HashMap<String, Vec<String>>,
) -> Result<(), String> {
    if qualifier_name == "user" {
        return Ok(());
    }

    if models.contains_key(qualifier_name) {
        return Ok(());
    }

    let is_known_model = models.values().flatten().any(|id| id == qualifier_name);

    if is_known_model {
        return Ok(());
    }

    Err(format!(
        "unknown qualifier '{qualifier_name}': not a provider or model"
    ))
}

// --- Internal ---

fn parse_yaml<T: serde::de::DeserializeOwned>(content: &str, label: &str) -> Result<T, String> {
    match serde_yaml::from_str(content) {
        Ok(parsed) => Ok(parsed),
        Err(err) => Err(format!("failed to parse {label}: {err}")),
    }
}

#[cfg(test)]
mod tests;
