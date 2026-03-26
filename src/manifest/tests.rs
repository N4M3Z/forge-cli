use super::*;
use extract::string_field;

const MANIFEST_FIXTURE: &str = include_str!("../../tests/fixtures/input/manifest-basic.yaml");
const MANIFEST_INVALID: &str = include_str!("../../tests/fixtures/input/manifest-invalid.yaml");

fn fixture() -> std::collections::HashMap<String, ManifestEntry> {
    read(MANIFEST_FIXTURE).expect("fixture should parse")
}

fn fixture_entry(name: &str) -> ManifestEntry {
    fixture()
        .remove(name)
        .unwrap_or_else(|| panic!("fixture missing {name}"))
}

// --- content_sha256 ---

#[test]
fn sha256_consistent() {
    let first = content_sha256("hello world");
    let second = content_sha256("hello world");
    assert_eq!(first, second);
    assert_eq!(first.len(), 64);
}

#[test]
fn sha256_different_inputs() {
    assert_ne!(content_sha256("aaa"), content_sha256("bbb"));
}

// --- generate_statement ---

#[test]
fn statement_is_valid_yaml() {
    let entry = fixture_entry("rules/AgentTeams.md");

    let statement = generate_statement(
        "rules/AgentTeams.md",
        &entry.sha256,
        &[("rules/AgentTeams.md".into(), content_sha256("source"))],
        "forge-cli",
        "https://github.com/N4M3Z/forge-cli/assemble/v1",
        env!("CARGO_PKG_VERSION"),
    );

    let parsed: serde_yaml::Value = serde_yaml::from_str(&statement).expect("should be valid YAML");
    let provenance = &parsed["provenance"];

    assert_eq!(
        provenance["_type"].as_str().unwrap(),
        "https://in-toto.io/Statement/v1"
    );
    assert_eq!(
        provenance["subject"][0]["name"].as_str().unwrap(),
        "rules/AgentTeams.md"
    );
}

#[test]
fn statement_includes_all_dependencies() {
    let inputs = vec![
        (
            "rules/AgentTeams.md".to_string(),
            content_sha256("source a"),
        ),
        (
            "rules/user/AgentTeams.md".to_string(),
            content_sha256("source b"),
        ),
    ];

    let statement = generate_statement(
        "rules/AgentTeams.md",
        &content_sha256("output"),
        &inputs,
        "forge-cli",
        "https://github.com/N4M3Z/forge-cli/assemble/v1",
        env!("CARGO_PKG_VERSION"),
    );

    let parsed: serde_yaml::Value = serde_yaml::from_str(&statement).unwrap();
    let provenance = &parsed["provenance"];
    let deps = provenance["predicate"]["buildDefinition"]["resolvedDependencies"]
        .as_sequence()
        .unwrap();

    assert_eq!(deps.len(), inputs.len());

    for (index, (uri, digest)) in inputs.iter().enumerate() {
        assert_eq!(deps[index]["uri"].as_str().unwrap(), uri);
        assert_eq!(deps[index]["digest"]["sha256"].as_str().unwrap(), digest);
    }
}

#[test]
fn statement_carries_builder_metadata() {
    let statement = generate_statement(
        "rules/CodeStyle.md",
        &content_sha256("output"),
        &[("rules/CodeStyle.md".into(), content_sha256("input"))],
        "test-builder",
        "https://example.com/build/v1",
        "1.2.3",
    );

    let parsed: serde_yaml::Value = serde_yaml::from_str(&statement).unwrap();
    let provenance = &parsed["provenance"];
    let run_details = &provenance["predicate"]["runDetails"];

    assert_eq!(
        run_details["builder"]["id"].as_str().unwrap(),
        "test-builder"
    );
    assert_eq!(
        run_details["builder"]["version"]["forge"].as_str().unwrap(),
        "1.2.3"
    );
    assert!(run_details["metadata"]["startedOn"].as_str().is_some());
}

// --- read ---

#[test]
fn read_parses_all_entries() {
    let entries = fixture();
    assert!(entries.contains_key("rules/AgentTeams.md"));
    assert!(entries.contains_key("rules/CodeStyle.md"));
}

#[test]
fn read_rejects_missing_sha256() {
    assert!(read(MANIFEST_INVALID).is_err());
}

// --- write ---

#[test]
fn write_roundtrips() {
    let mut entries = std::collections::HashMap::new();
    entries.insert(
        "agents/Helper.md".to_string(),
        ManifestEntry {
            sha256: content_sha256("output content"),
        },
    );

    let yaml = write(&entries).expect("write should succeed");
    let roundtrip = read(&yaml).expect("roundtrip read should succeed");

    assert!(roundtrip.contains_key("agents/Helper.md"));
    assert_eq!(
        roundtrip["agents/Helper.md"].sha256,
        entries["agents/Helper.md"].sha256
    );
}

// --- check_sources ---

#[test]
fn check_sources_unchanged_when_matching() {
    let sources = vec![
        ("rules/A.md".to_string(), content_sha256("content a")),
        ("rules/B.md".to_string(), content_sha256("content b")),
    ];
    assert_eq!(check_sources(&sources, &sources), FileStatus::Unchanged);
}

#[test]
fn check_sources_stale_when_hash_differs() {
    let stored = vec![("rules/A.md".to_string(), content_sha256("old"))];
    let current = vec![("rules/A.md".to_string(), content_sha256("new"))];
    assert_eq!(check_sources(&stored, &current), FileStatus::Stale);
}

#[test]
fn check_sources_stale_when_file_added() {
    let stored = vec![("rules/A.md".to_string(), content_sha256("a"))];
    let current = vec![
        ("rules/A.md".to_string(), content_sha256("a")),
        ("rules/B.md".to_string(), content_sha256("b")),
    ];
    assert_eq!(check_sources(&stored, &current), FileStatus::Stale);
}

#[test]
fn check_sources_stale_when_file_renamed() {
    let stored = vec![("rules/Old.md".to_string(), content_sha256("a"))];
    let current = vec![("rules/New.md".to_string(), content_sha256("a"))];
    assert_eq!(check_sources(&stored, &current), FileStatus::Stale);
}

// --- status ---

#[test]
fn status_new_when_no_manifest_entry() {
    assert_eq!(status(Some("content"), None, "abc"), FileStatus::New);
}

#[test]
fn status_new_when_target_missing() {
    let entry = ManifestEntry {
        sha256: content_sha256("content"),
    };
    assert_eq!(status(None, Some(&entry), "abc"), FileStatus::New);
}

#[test]
fn status_modified_when_target_edited() {
    let entry = ManifestEntry {
        sha256: content_sha256("original"),
    };
    let build_sha256 = content_sha256("original");
    assert_eq!(
        status(Some("user edited this"), Some(&entry), &build_sha256),
        FileStatus::Modified
    );
}

#[test]
fn status_stale_when_source_changed() {
    let deployed_sha256 = content_sha256("old build");
    let entry = ManifestEntry {
        sha256: deployed_sha256.clone(),
    };
    let new_build_sha256 = content_sha256("new build");
    assert_eq!(
        status(Some("old build"), Some(&entry), &new_build_sha256),
        FileStatus::Stale
    );
}

#[test]
fn status_unchanged_when_all_match() {
    let content = "assembled output";
    let sha256 = content_sha256(content);
    let entry = ManifestEntry {
        sha256: sha256.clone(),
    };
    assert_eq!(
        status(Some(content), Some(&entry), &sha256),
        FileStatus::Unchanged
    );
}

// --- extract::string_field ---

#[test]
fn string_field_returns_value() {
    let yaml: serde_yaml::Value = serde_yaml::from_str("name: Alice").unwrap();
    assert_eq!(string_field(&yaml, "name", "test").unwrap(), "Alice");
}

#[test]
fn string_field_error_when_missing() {
    let yaml: serde_yaml::Value = serde_yaml::from_str("name: Alice").unwrap();
    assert!(string_field(&yaml, "age", "test").is_err());
}

#[test]
fn string_field_error_when_not_string() {
    let yaml: serde_yaml::Value = serde_yaml::from_str("count: 42").unwrap();
    assert!(string_field(&yaml, "count", "test").is_err());
}
