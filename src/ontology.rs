use crate::error::{Error, ErrorKind};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Once;

static PROJECT_WARNING: Once = Once::new();

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub ontology: Ontology,
    pub extensions: Vec<String>,
    pub launch: Launch,
    pub watch: Watch,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct Ontology {
    pub workshop: Option<String>,
    pub owner: Option<String>,
    pub archive: Option<String>,
    pub vault: Option<String>,
    pub work: Option<String>,
    pub data: Option<String>,
    pub mount: Option<String>,
    pub developer: Option<String>,
    pub documents: Option<String>,
    pub githooks: Option<String>,
    pub domain: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct Launch {}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct Watch {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Env,
    Config,
    Default,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResolvedField {
    pub key: &'static str,
    pub env: &'static str,
    pub value: Option<String>,
    pub source: Option<Source>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ResolvedOntology {
    pub workshop: Option<ResolvedValue>,
    pub owner: Option<ResolvedValue>,
    pub archive: Option<ResolvedValue>,
    pub vault: Option<ResolvedValue>,
    pub work: Option<ResolvedValue>,
    pub data: Option<ResolvedValue>,
    pub mount: Option<ResolvedValue>,
    pub developer: Option<ResolvedValue>,
    pub documents: Option<ResolvedValue>,
    pub githooks: Option<ResolvedValue>,
    pub domain: Option<ResolvedValue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResolvedValue {
    pub value: String,
    pub source: Source,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ResolvedConfig {
    pub ontology: ResolvedOntology,
    pub extensions: Vec<PathBuf>,
}

#[derive(Debug, Clone, Copy)]
enum Key {
    Workshop,
    Owner,
    Archive,
    Vault,
    Work,
    Data,
    Mount,
    Developer,
    Documents,
    Githooks,
    Domain,
}

impl Key {
    const ALL: [Self; 11] = [
        Self::Workshop,
        Self::Owner,
        Self::Archive,
        Self::Vault,
        Self::Work,
        Self::Data,
        Self::Mount,
        Self::Developer,
        Self::Documents,
        Self::Githooks,
        Self::Domain,
    ];

    fn name(self) -> &'static str {
        match self {
            Self::Workshop => "workshop",
            Self::Owner => "owner",
            Self::Archive => "archive",
            Self::Vault => "vault",
            Self::Work => "work",
            Self::Data => "data",
            Self::Mount => "mount",
            Self::Developer => "developer",
            Self::Documents => "documents",
            Self::Githooks => "githooks",
            Self::Domain => "domain",
        }
    }

    fn env(self) -> &'static str {
        match self {
            Self::Workshop => "FORGE_WORKSHOP",
            Self::Owner => "FORGE_OWNER",
            Self::Archive => "FORGE_ARCHIVE",
            Self::Vault => "FORGE_VAULT",
            Self::Work => "FORGE_WORK",
            Self::Data => "FORGE_DATA",
            Self::Mount => "FORGE_MOUNT",
            Self::Developer => "FORGE_DEVELOPER",
            Self::Documents => "FORGE_DOCUMENTS",
            Self::Githooks => "FORGE_GITHOOKS",
            Self::Domain => "FORGE_DOMAIN",
        }
    }

    fn default(self) -> Option<&'static str> {
        match self {
            Self::Workshop => Some("~/Agents"),
            Self::Archive => Some("~/Agents/archive"),
            Self::Vault => Some("~/Atlas/Domains"),
            Self::Data => Some("~/Data"),
            Self::Mount => Some("~/Atlas"),
            Self::Domain => Some("Technology"),
            Self::Owner | Self::Work | Self::Developer | Self::Documents | Self::Githooks => None,
        }
    }

    fn configured(self, ontology: &Ontology) -> Option<&String> {
        match self {
            Self::Workshop => ontology.workshop.as_ref(),
            Self::Owner => ontology.owner.as_ref(),
            Self::Archive => ontology.archive.as_ref(),
            Self::Vault => ontology.vault.as_ref(),
            Self::Work => ontology.work.as_ref(),
            Self::Data => ontology.data.as_ref(),
            Self::Mount => ontology.mount.as_ref(),
            Self::Developer => ontology.developer.as_ref(),
            Self::Documents => ontology.documents.as_ref(),
            Self::Githooks => ontology.githooks.as_ref(),
            Self::Domain => ontology.domain.as_ref(),
        }
    }

    fn is_path(self) -> bool {
        !matches!(self, Self::Owner | Self::Domain)
    }
}

impl ResolvedOntology {
    pub fn fields(&self) -> Vec<ResolvedField> {
        Key::ALL
            .iter()
            .map(|key| {
                let resolved = self.value(*key);
                ResolvedField {
                    key: key.name(),
                    env: key.env(),
                    value: resolved.map(|value| value.value.clone()),
                    source: resolved.map(|value| value.source),
                }
            })
            .collect()
    }

    fn value(&self, key: Key) -> Option<&ResolvedValue> {
        match key {
            Key::Workshop => self.workshop.as_ref(),
            Key::Owner => self.owner.as_ref(),
            Key::Archive => self.archive.as_ref(),
            Key::Vault => self.vault.as_ref(),
            Key::Work => self.work.as_ref(),
            Key::Data => self.data.as_ref(),
            Key::Mount => self.mount.as_ref(),
            Key::Developer => self.developer.as_ref(),
            Key::Documents => self.documents.as_ref(),
            Key::Githooks => self.githooks.as_ref(),
            Key::Domain => self.domain.as_ref(),
        }
    }
}

pub fn load() -> Result<ResolvedConfig, Error> {
    let config_dir = config_dir()?;
    load_from_dir_with_env(&config_dir, &|name| std::env::var(name).ok())
}

pub fn config_dir() -> Result<PathBuf, Error> {
    dirs::home_dir()
        .map(|home| home.join(".config/forge"))
        .ok_or_else(|| Error::new(ErrorKind::Config, "cannot resolve home directory"))
}

pub fn env_vars(config: &ResolvedConfig) -> Vec<(String, String)> {
    config
        .ontology
        .fields()
        .into_iter()
        .filter_map(|field| {
            field
                .value
                .map(|value| (field.env.to_string(), value.clone()))
        })
        .collect()
}

pub fn expand_tilde(value: &str) -> PathBuf {
    if let Some(rest) = value.strip_prefix("~/") {
        return dirs::home_dir().map_or_else(|| PathBuf::from(value), |home| home.join(rest));
    }
    PathBuf::from(value)
}

fn load_from_dir_with_env(
    config_dir: &Path,
    env: &dyn Fn(&str) -> Option<String>,
) -> Result<ResolvedConfig, Error> {
    let config = load_raw_config(config_dir)?;
    Ok(resolve_config(&config, env))
}

fn load_raw_config(config_dir: &Path) -> Result<Config, Error> {
    let config_path = config_dir.join("config.yaml");
    match fs::read_to_string(&config_path) {
        Ok(content) => parse_config(&content, &config_path),
        Err(error) if error.kind() == io::ErrorKind::NotFound => load_project_fallback(config_dir),
        Err(error) => Err(Error::new(
            ErrorKind::Io,
            format!("cannot read {}: {error}", config_path.display()),
        )),
    }
}

fn parse_config(content: &str, path: &Path) -> Result<Config, Error> {
    serde_yaml::from_str(content).map_err(|error| {
        Error::new(
            ErrorKind::Config,
            format!("{} is malformed: {error}", path.display()),
        )
    })
}

fn load_project_fallback(config_dir: &Path) -> Result<Config, Error> {
    let project_path = config_dir.join("project.yaml");
    let content = match fs::read_to_string(&project_path) {
        Ok(content) => content,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Config::default()),
        Err(error) => {
            return Err(Error::new(
                ErrorKind::Io,
                format!("cannot read {}: {error}", project_path.display()),
            ));
        }
    };
    PROJECT_WARNING.call_once(|| {
        eprintln!(
            "warning: {} is deprecated; migrate to config.yaml",
            project_path.display()
        );
    });
    let project: ProjectConfig = serde_yaml::from_str(&content).map_err(|error| {
        Error::new(
            ErrorKind::Config,
            format!("{} is malformed: {error}", project_path.display()),
        )
    })?;
    Ok(Config {
        ontology: project.into_ontology(),
        ..Config::default()
    })
}

fn resolve_config(config: &Config, env: &dyn Fn(&str) -> Option<String>) -> ResolvedConfig {
    let ontology = ResolvedOntology {
        workshop: resolve_key(Key::Workshop, &config.ontology, env),
        owner: resolve_key(Key::Owner, &config.ontology, env),
        archive: resolve_key(Key::Archive, &config.ontology, env),
        vault: resolve_key(Key::Vault, &config.ontology, env),
        work: resolve_key(Key::Work, &config.ontology, env),
        data: resolve_key(Key::Data, &config.ontology, env),
        mount: resolve_key(Key::Mount, &config.ontology, env),
        developer: resolve_key(Key::Developer, &config.ontology, env),
        documents: resolve_key(Key::Documents, &config.ontology, env),
        githooks: resolve_key(Key::Githooks, &config.ontology, env),
        domain: resolve_key(Key::Domain, &config.ontology, env),
    };
    let extensions = config
        .extensions
        .iter()
        .map(|extension| expand_tilde(extension))
        .collect();
    ResolvedConfig {
        ontology,
        extensions,
    }
}

fn resolve_key(
    key: Key,
    ontology: &Ontology,
    env: &dyn Fn(&str) -> Option<String>,
) -> Option<ResolvedValue> {
    if let Some(value) = env(key.env()) {
        return Some(resolved_value(key, value, Source::Env));
    }
    if let Some(value) = key.configured(ontology) {
        return Some(resolved_value(key, value.clone(), Source::Config));
    }
    key.default()
        .map(|value| resolved_value(key, value.to_string(), Source::Default))
}

fn resolved_value(key: Key, value: String, source: Source) -> ResolvedValue {
    let value = if key.is_path() {
        expand_tilde(&value).to_string_lossy().to_string()
    } else {
        value
    };
    ResolvedValue { value, source }
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct ProjectConfig {
    workshop: Option<String>,
    owner: Option<String>,
    archive: Option<String>,
    vault: Option<String>,
    work: Option<String>,
    data: Option<String>,
    mount: Option<String>,
    githooks: Option<String>,
    developer: Option<String>,
    documents: Option<String>,
    defaults: ProjectDefaults,
    exclude: Vec<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct ProjectDefaults {
    domain: Option<String>,
}

impl ProjectConfig {
    fn into_ontology(self) -> Ontology {
        let _ = self.exclude;
        Ontology {
            workshop: self.workshop,
            owner: self.owner,
            archive: self.archive,
            vault: self.vault,
            work: self.work,
            data: self.data,
            mount: self.mount,
            developer: self.developer,
            documents: self.documents,
            githooks: self.githooks,
            domain: self.defaults.domain,
        }
    }
}

#[cfg(test)]
mod tests;
