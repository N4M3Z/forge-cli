use super::*;
use tempfile::TempDir;

const MODULE_YAML: &str = concat!(
    "name: test-module\n",
    "version: 0.1.0\n",
    "description: test\n",
    "events: []\n",
    "repository: https://github.com/test/repo\n",
);

fn write_module_yaml(module_root: &std::path::Path) {
    std::fs::write(module_root.join("module.yaml"), MODULE_YAML).unwrap();
}

#[test]
fn execute_copies_markdown_files() {
    let source = TempDir::new().unwrap();
    let target = TempDir::new().unwrap();

    let rules_directory = source.path().join("rules");
    std::fs::create_dir_all(&rules_directory).unwrap();
    std::fs::write(rules_directory.join("Test.md"), "# Test rule").unwrap();

    let result = execute(
        &source.path().to_string_lossy(),
        &target.path().to_string_lossy(),
        true,
    )
    .unwrap();

    assert_eq!(result.installed.len(), 1);
    assert!(target.path().join("rules/Test.md").exists());
}

#[test]
fn execute_skips_non_markdown_files() {
    let source = TempDir::new().unwrap();
    let target = TempDir::new().unwrap();

    let rules_directory = source.path().join("rules");
    std::fs::create_dir_all(&rules_directory).unwrap();
    std::fs::write(rules_directory.join("data.yaml"), "key: value").unwrap();

    let result = execute(
        &source.path().to_string_lossy(),
        &target.path().to_string_lossy(),
        true,
    )
    .unwrap();

    assert!(result.installed.is_empty());
    assert!(!target.path().join("rules/data.yaml").exists());
}

#[test]
fn execute_copies_nested_directories() {
    let source = TempDir::new().unwrap();
    let target = TempDir::new().unwrap();

    let nested = source.path().join("rules/cz");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::write(nested.join("Tax.md"), "# Tax rule").unwrap();

    let result = execute(
        &source.path().to_string_lossy(),
        &target.path().to_string_lossy(),
        true,
    )
    .unwrap();

    assert_eq!(result.installed.len(), 1);
    assert!(target.path().join("rules/cz/Tax.md").exists());
}

#[test]
fn execute_empty_module_succeeds() {
    let source = TempDir::new().unwrap();
    let target = TempDir::new().unwrap();

    let result = execute(
        &source.path().to_string_lossy(),
        &target.path().to_string_lossy(),
        true,
    )
    .unwrap();

    assert!(result.installed.is_empty());
}

#[test]
fn execute_writes_provenance_sidecar_by_default() {
    let source = TempDir::new().unwrap();
    let target = TempDir::new().unwrap();

    write_module_yaml(source.path());
    let rules_directory = source.path().join("rules");
    std::fs::create_dir_all(&rules_directory).unwrap();
    std::fs::write(rules_directory.join("KeepChangelog.md"), "# Keep changelog").unwrap();

    execute(
        &source.path().to_string_lossy(),
        &target.path().to_string_lossy(),
        false,
    )
    .unwrap();

    let sidecar = target.path().join("rules/.provenance/KeepChangelog.yaml");
    assert!(
        sidecar.exists(),
        "expected provenance sidecar at {}",
        sidecar.display()
    );

    let statement = std::fs::read_to_string(&sidecar).unwrap();
    let parsed: serde_yaml::Value = serde_yaml::from_str(&statement).unwrap();
    let provenance = &parsed["provenance"];

    assert_eq!(
        provenance["predicate"]["buildDefinition"]["buildType"]
            .as_str()
            .unwrap(),
        &format!("{}/copy/v1", env!("CARGO_PKG_REPOSITORY"))
    );
    assert_eq!(
        provenance["predicate"]["buildDefinition"]["externalParameters"]["source"]
            .as_str()
            .unwrap(),
        "https://github.com/test/repo"
    );
    assert_eq!(
        provenance["subject"][0]["name"].as_str().unwrap(),
        "rules/KeepChangelog.md"
    );
}

#[test]
fn execute_skips_provenance_when_flag_set() {
    let source = TempDir::new().unwrap();
    let target = TempDir::new().unwrap();

    write_module_yaml(source.path());
    let rules_directory = source.path().join("rules");
    std::fs::create_dir_all(&rules_directory).unwrap();
    std::fs::write(rules_directory.join("Foo.md"), "# Foo").unwrap();

    execute(
        &source.path().to_string_lossy(),
        &target.path().to_string_lossy(),
        true,
    )
    .unwrap();

    assert!(target.path().join("rules/Foo.md").exists());
    assert!(!target.path().join("rules/.provenance").exists());
}

#[test]
fn execute_skips_provenance_when_no_module_yaml() {
    let source = TempDir::new().unwrap();
    let target = TempDir::new().unwrap();

    let rules_directory = source.path().join("rules");
    std::fs::create_dir_all(&rules_directory).unwrap();
    std::fs::write(rules_directory.join("Foo.md"), "# Foo").unwrap();

    execute(
        &source.path().to_string_lossy(),
        &target.path().to_string_lossy(),
        false,
    )
    .unwrap();

    assert!(target.path().join("rules/Foo.md").exists());
    assert!(!target.path().join("rules/.provenance").exists());
}

#[test]
fn execute_provenance_digest_matches_content() {
    let source = TempDir::new().unwrap();
    let target = TempDir::new().unwrap();

    write_module_yaml(source.path());
    let rules_directory = source.path().join("rules");
    std::fs::create_dir_all(&rules_directory).unwrap();
    let content = "# Rule body\n";
    std::fs::write(rules_directory.join("Rule.md"), content).unwrap();

    execute(
        &source.path().to_string_lossy(),
        &target.path().to_string_lossy(),
        false,
    )
    .unwrap();

    let statement =
        std::fs::read_to_string(target.path().join("rules/.provenance/Rule.yaml")).unwrap();
    let parsed: serde_yaml::Value = serde_yaml::from_str(&statement).unwrap();

    let expected_digest = commands::manifest::content_sha256(content);
    assert_eq!(
        parsed["provenance"]["subject"][0]["digest"]["sha256"]
            .as_str()
            .unwrap(),
        expected_digest
    );
}

#[test]
fn execute_resists_yaml_injection() {
    let source = TempDir::new().unwrap();
    let target = TempDir::new().unwrap();

    let hostile_module_yaml = "name: hostile\nversion: 0.1.0\ndescription: pwn\nevents: []\nrepository: \"https://x.test/repo\\nbogus_root_key: pwned\"\n";
    std::fs::write(source.path().join("module.yaml"), hostile_module_yaml).unwrap();

    let rules_directory = source.path().join("rules");
    std::fs::create_dir_all(&rules_directory).unwrap();
    std::fs::write(rules_directory.join("Foo.md"), "# Foo").unwrap();

    execute(
        &source.path().to_string_lossy(),
        &target.path().to_string_lossy(),
        false,
    )
    .unwrap();

    let sidecar_text = std::fs::read_to_string(target.path().join("rules/.provenance/Foo.yaml"))
        .expect("sidecar must exist");
    let mapping: serde_yaml::Mapping = serde_yaml::from_str(&sidecar_text).expect("valid YAML");

    let top_level_keys: Vec<String> = mapping
        .keys()
        .map(|k| k.as_str().unwrap_or_default().to_string())
        .collect();
    assert_eq!(
        top_level_keys,
        vec!["provenance".to_string()],
        "injection inserted top-level keys: {top_level_keys:?}"
    );

    let sidecar = commands::manifest::provenance::parse(&sidecar_text).expect("typed parse");
    assert_eq!(
        sidecar
            .provenance
            .predicate
            .build_definition
            .external_parameters
            .source,
        "https://x.test/repo\nbogus_root_key: pwned",
        "hostile string must round-trip as opaque scalar"
    );
}

#[test]
fn execute_writes_posix_paths() {
    let source = TempDir::new().unwrap();
    let target = TempDir::new().unwrap();

    write_module_yaml(source.path());
    let nested = source.path().join("rules").join("cz");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::write(nested.join("Tax.md"), "# Tax rule").unwrap();

    execute(
        &source.path().to_string_lossy(),
        &target.path().to_string_lossy(),
        false,
    )
    .unwrap();

    let sidecar_text =
        std::fs::read_to_string(target.path().join("rules/cz/.provenance/Tax.yaml")).unwrap();
    let sidecar = commands::manifest::provenance::parse(&sidecar_text).unwrap();

    assert_eq!(sidecar.provenance.subject[0].name, "rules/cz/Tax.md");
    assert_eq!(
        sidecar
            .provenance
            .predicate
            .build_definition
            .resolved_dependencies[0]
            .uri,
        "rules/cz/Tax.md"
    );
}

#[test]
fn execute_sidecar_round_trips_typed() {
    let source = TempDir::new().unwrap();
    let target = TempDir::new().unwrap();

    write_module_yaml(source.path());
    let rules_directory = source.path().join("rules");
    std::fs::create_dir_all(&rules_directory).unwrap();
    std::fs::write(rules_directory.join("Roundtrip.md"), "# body\n").unwrap();

    execute(
        &source.path().to_string_lossy(),
        &target.path().to_string_lossy(),
        false,
    )
    .unwrap();

    let sidecar_path = target.path().join("rules/.provenance/Roundtrip.yaml");
    let sidecar = commands::manifest::provenance::read(&sidecar_path).expect("typed parse");

    assert_eq!(
        sidecar.provenance.statement_type,
        "https://in-toto.io/Statement/v1"
    );
    assert_eq!(sidecar.provenance.subject.len(), 1);
    assert_eq!(sidecar.provenance.subject[0].name, "rules/Roundtrip.md");
    assert!(!sidecar.provenance.subject[0].digest.sha256.is_empty());

    let build = &sidecar.provenance.predicate.build_definition;
    assert!(build.build_type.ends_with("/copy/v1"));
    assert_eq!(
        build.external_parameters.source,
        "https://github.com/test/repo"
    );
    assert_eq!(build.resolved_dependencies.len(), 1);
    assert_eq!(build.resolved_dependencies[0].uri, "rules/Roundtrip.md");
    assert_eq!(
        build.resolved_dependencies[0].digest.sha256,
        sidecar.provenance.subject[0].digest.sha256
    );

    let runner = &sidecar.provenance.predicate.run_details;
    assert_eq!(runner.builder.id, env!("CARGO_PKG_NAME"));
    assert_eq!(runner.builder.version.forge, env!("CARGO_PKG_VERSION"));
    assert!(!runner.metadata.started_on.is_empty());
}
