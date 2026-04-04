use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct ModuleManifest {
    pub name: String,
    pub version: String,
    pub description: String,
    #[serde(default)]
    pub events: Vec<String>,
    #[serde(rename = "type")]
    pub module_type: Option<String>,
    pub platforms: Option<Vec<String>>,
    pub repository: Option<String>,
}

impl ModuleManifest {
    pub fn source_uri(&self) -> &str {
        self.repository.as_deref().unwrap_or(&self.name)
    }
}

pub fn load(module_root: &Path) -> Result<ModuleManifest, String> {
    let module_yaml = module_root.join("module.yaml");
    let content = std::fs::read_to_string(&module_yaml)
        .map_err(|error| format!("cannot read {}: {error}", module_yaml.display()))?;

    serde_yaml::from_str(&content).map_err(|error| format!("invalid module.yaml: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_uri_prefers_repository() {
        let manifest = ModuleManifest {
            name: "forge-core".to_string(),
            version: "0.5.0".to_string(),
            description: "test".to_string(),
            events: Vec::new(),
            module_type: None,
            platforms: None,
            repository: Some("https://github.com/N4M3Z/forge-core".to_string()),
        };
        assert_eq!(manifest.source_uri(), "https://github.com/N4M3Z/forge-core");
    }

    #[test]
    fn source_uri_falls_back_to_name() {
        let manifest = ModuleManifest {
            name: "forge-gm".to_string(),
            version: "0.1.0".to_string(),
            description: "test".to_string(),
            events: Vec::new(),
            module_type: None,
            platforms: None,
            repository: None,
        };
        assert_eq!(manifest.source_uri(), "forge-gm");
    }

    #[test]
    fn deserializes_minimal_module_yaml() {
        let yaml_content = "name: test-module\nversion: 0.1.0\ndescription: test\nevents: []\n";
        let manifest: ModuleManifest = serde_yaml::from_str(yaml_content).unwrap();
        assert_eq!(manifest.name, "test-module");
        assert!(manifest.repository.is_none());
        assert!(manifest.module_type.is_none());
    }

    #[test]
    fn deserializes_full_module_yaml() {
        let yaml_content = "name: forge-cli\nversion: 0.1.0\ntype: binary\ndescription: test\nevents: []\nrepository: https://github.com/test/repo\nplatforms: [macos, linux]\n";
        let manifest: ModuleManifest = serde_yaml::from_str(yaml_content).unwrap();
        assert_eq!(manifest.module_type.as_deref(), Some("binary"));
        assert_eq!(manifest.platforms.as_ref().unwrap().len(), 2);
    }
}
