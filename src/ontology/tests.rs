use super::*;

fn no_env(_: &str) -> Option<String> {
    None
}

fn env_from<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
    |name| {
        pairs
            .iter()
            .find_map(|(key, value)| (*key == name).then(|| (*value).to_string()))
    }
}

#[test]
fn env_beats_config_beats_default() {
    let config = Config {
        ontology: Ontology {
            workshop: Some("/from-config".to_string()),
            ..Ontology::default()
        },
        ..Config::default()
    };
    let resolved = resolve_config(&config, &env_from(&[("FORGE_WORKSHOP", "/from-env")]));
    let workshop = resolved.ontology.workshop.expect("workshop resolved");
    assert_eq!(workshop.value, "/from-env");
    assert_eq!(workshop.source, Source::Env);
}

#[test]
fn config_beats_default() {
    let config = Config {
        ontology: Ontology {
            domain: Some("Research".to_string()),
            ..Ontology::default()
        },
        ..Config::default()
    };
    let resolved = resolve_config(&config, &no_env);
    let domain = resolved.ontology.domain.expect("domain resolved");
    assert_eq!(domain.value, "Research");
    assert_eq!(domain.source, Source::Config);
}

#[test]
fn missing_config_uses_default() {
    let resolved = resolve_config(&Config::default(), &no_env);
    let domain = resolved.ontology.domain.expect("domain default");
    assert_eq!(domain.value, "Technology");
    assert_eq!(domain.source, Source::Default);
}

#[test]
fn env_override_is_reflected_in_env_vars() {
    let resolved = resolve_config(
        &Config::default(),
        &env_from(&[("FORGE_OWNER", "N4M3Z"), ("FORGE_DOMAIN", "Systems")]),
    );
    let vars = env_vars(&resolved);
    assert!(vars.contains(&("FORGE_OWNER".to_string(), "N4M3Z".to_string())));
    assert!(vars.contains(&("FORGE_DOMAIN".to_string(), "Systems".to_string())));
}

#[test]
fn tilde_expansion_resolves_under_home() {
    let home = dirs::home_dir().expect("home for test");
    assert_eq!(expand_tilde("~/Agents"), home.join("Agents"));
}

#[test]
fn project_yaml_fallback_populates_ontology() {
    let dir = tempfile::tempdir().expect("tempdir");
    let project = dir.path().join("project.yaml");
    std::fs::write(
        &project,
        "workshop: /workshop\ndefaults:\n    domain: Security\n",
    )
    .expect("write project");

    let resolved = load_from_dir_with_env(dir.path(), &no_env).expect("load project fallback");
    let workshop = resolved.ontology.workshop.expect("workshop");
    let domain = resolved.ontology.domain.expect("domain");
    assert_eq!(workshop.value, "/workshop");
    assert_eq!(workshop.source, Source::Config);
    assert_eq!(domain.value, "Security");
}

#[test]
fn unknown_top_level_config_key_errors() {
    let dir = tempfile::tempdir().expect("tempdir");
    let config = dir.path().join("config.yaml");
    std::fs::write(&config, "surprise: true\n").expect("write config");

    let error = load_from_dir_with_env(dir.path(), &no_env).expect_err("unknown key");
    assert_eq!(error.kind(), ErrorKind::Config);
    assert!(error.message().contains("unknown field"), "got: {error}");
}
