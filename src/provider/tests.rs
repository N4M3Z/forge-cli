use super::*;

const DEFAULTS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/configs/defaults-basic.yaml"
));

const MODELS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/configs/models-basic.yaml"
));

#[test]
fn load_providers_parses_all_providers() {
    let providers = load_providers(DEFAULTS).unwrap();

    assert!(providers.contains_key("claude"));
    assert!(providers.contains_key("gemini"));
    assert!(providers.contains_key("codex"));
    assert!(providers.contains_key("opencode"));
}

#[test]
fn load_providers_reads_target() {
    let providers = load_providers(DEFAULTS).unwrap();

    assert_eq!(providers["claude"].target, ".claude");
    assert_eq!(providers["gemini"].target, ".gemini");
}

#[test]
fn load_providers_reads_assembly_steps() {
    let providers = load_providers(DEFAULTS).unwrap();

    let gemini = &providers["gemini"];
    let assembly = gemini.assembly.as_ref().unwrap();
    assert_eq!(assembly.len(), 2);
    assert_eq!(assembly[0], "kebab-case-agents");
    assert_eq!(assembly[1], "remap-tools");

    let claude = &providers["claude"];
    assert!(claude.assembly.is_none());
}

#[test]
fn load_providers_deploy_is_none_when_absent() {
    let providers = load_providers(DEFAULTS).unwrap();

    let claude = &providers["claude"];
    assert!(claude.deploy.is_none());
}

#[test]
fn load_providers_reads_deploy_steps() {
    let providers = load_providers(DEFAULTS).unwrap();

    let codex = &providers["codex"];
    let deploy = codex.deploy.as_ref().unwrap();
    assert_eq!(deploy.len(), 1);
    assert_eq!(deploy[0], "rulesync");
}

#[test]
fn load_providers_rejects_invalid_yaml() {
    let result = load_providers("not: valid: yaml: {{");
    assert!(result.is_err());
}

#[test]
fn assembly_rule_from_name_accepts_known_rules() {
    assert_eq!(
        AssemblyRule::from_name("kebab-case").unwrap(),
        AssemblyRule::KebabCase,
    );
    assert_eq!(
        AssemblyRule::from_name("kebab-case-agents").unwrap(),
        AssemblyRule::KebabCaseAgents,
    );
    assert_eq!(
        AssemblyRule::from_name("remap-tools").unwrap(),
        AssemblyRule::RemapTools,
    );
    assert_eq!(
        AssemblyRule::from_name("agents-to-toml").unwrap(),
        AssemblyRule::AgentsToToml,
    );
}

#[test]
fn assembly_rule_from_name_rejects_unknown() {
    let result = AssemblyRule::from_name("nonexistent");
    assert!(result.is_err());
}

#[test]
fn load_models_parses_providers_and_model_ids() {
    let models = load_models(MODELS).unwrap();

    assert!(models.contains_key("claude"));
    assert!(models.contains_key("codex"));
    assert!(models.contains_key("gemini"));

    let claude_models = &models["claude"];
    assert!(claude_models.contains(&"claude-opus-4-6".to_string()));
    assert!(claude_models.contains(&"claude-sonnet-4-6".to_string()));
}

#[test]
fn validate_qualifier_accepts_provider_name() {
    let models = load_models(MODELS).unwrap();

    assert!(validate_qualifier("claude", &models).is_ok());
    assert!(validate_qualifier("gemini", &models).is_ok());
}

#[test]
fn validate_qualifier_accepts_model_id() {
    let models = load_models(MODELS).unwrap();

    assert!(validate_qualifier("claude-opus-4-6", &models).is_ok());
    assert!(validate_qualifier("o4-mini", &models).is_ok());
}

#[test]
fn validate_qualifier_always_accepts_user() {
    let models = load_models(MODELS).unwrap();

    assert!(validate_qualifier("user", &models).is_ok());
}

#[test]
fn validate_qualifier_user_valid_with_empty_models() {
    let empty: HashMap<String, Vec<String>> = HashMap::new();

    assert!(validate_qualifier("user", &empty).is_ok());
}

#[test]
fn validate_qualifier_rejects_unknown() {
    let models = load_models(MODELS).unwrap();

    let result = validate_qualifier("gpt-5", &models);
    assert!(result.is_err());
}

#[test]
fn map_tool_returns_mapped_value() {
    let mut mappings = HashMap::new();
    mappings.insert("Read".to_string(), "ReadFile".to_string());

    assert_eq!(map_tool("Read", &mappings), "ReadFile");
}

#[test]
fn map_tool_passes_through_unmapped() {
    let mappings: HashMap<String, String> = HashMap::new();

    assert_eq!(map_tool("Write", &mappings), "Write");
}

fn provider_with_aliases(target: &str, aliases: Vec<&str>) -> ProviderConfig {
    ProviderConfig {
        target: target.to_string(),
        assembly: None,
        deploy: None,
        keep_fields: None,
        models: None,
        aliases: Some(aliases.into_iter().map(String::from).collect()),
    }
}

#[test]
fn matches_target_by_provider_key() {
    let config = provider_with_aliases(".claude", vec!["claudecode"]);
    assert!(config.matches_target("claude", "claude"));
}

#[test]
fn matches_target_by_alias() {
    let config = provider_with_aliases(".claude", vec!["claudecode"]);
    assert!(config.matches_target("claudecode", "claude"));
}

#[test]
fn matches_target_by_target_directory() {
    let config = provider_with_aliases(".claude", vec!["claudecode"]);
    assert!(config.matches_target(".claude", "claude"));
}

#[test]
fn matches_target_by_stripped_dot_prefix() {
    let config = provider_with_aliases(".gemini", vec!["geminicli"]);
    assert!(config.matches_target("gemini", "gemini"));
}

#[test]
fn matches_target_rejects_unknown() {
    let config = provider_with_aliases(".claude", vec!["claudecode"]);
    assert!(!config.matches_target("cursor", "claude"));
}

#[test]
fn matches_target_no_aliases() {
    let config = ProviderConfig {
        target: ".opencode".to_string(),
        assembly: None,
        deploy: None,
        keep_fields: None,
        models: None,
        aliases: None,
    };
    assert!(config.matches_target("opencode", "opencode"));
    assert!(!config.matches_target("claudecode", "opencode"));
}
