use super::parse::{Source, parse};

const MINIMAL: &str = r"
version: 1
sources:
    forge-core:
        path: ../forge-core
artifacts:
    forge-core:
        skills: [BuildSkill]
";

#[test]
fn parse_minimal_happy_path() {
    let manifest = parse(MINIMAL).expect("minimal manifest must parse");
    assert_eq!(manifest.version, 1);
    assert_eq!(manifest.sources.len(), 1);
    let Source::Local { path } = &manifest.sources["forge-core"];
    assert_eq!(path.to_string_lossy(), "../forge-core");
    assert_eq!(manifest.artifacts["forge-core"].skills, vec!["BuildSkill"]);
    assert!(manifest.artifacts["forge-core"].agents.is_empty());
    assert!(manifest.artifacts["forge-core"].rules.is_empty());
}

#[test]
fn parse_full_artifact_list() {
    let content = r"
version: 1
sources:
    a:
        path: ./a
artifacts:
    a:
        skills: [S1, S2]
        agents: [A1]
        rules: [R1, R2, R3]
";
    let manifest = parse(content).unwrap();
    assert_eq!(manifest.artifacts["a"].skills, vec!["S1", "S2"]);
    assert_eq!(manifest.artifacts["a"].agents, vec!["A1"]);
    assert_eq!(manifest.artifacts["a"].rules.len(), 3);
}

#[test]
fn parse_rejects_unknown_top_level_field() {
    let content = r"
version: 1
sources:
    a:
        path: ./a
typo_field: oops
";
    let error = parse(content).expect_err("unknown field must error");
    assert!(
        error.to_string().contains("typo_field"),
        "error must name the offending field: {error}"
    );
}

#[test]
fn parse_rejects_unknown_source_field() {
    let content = r"
version: 1
sources:
    a:
        path: ./a
        bogus_key: 42
";
    // serde's untagged-enum error message says "did not match any variant"
    // rather than naming the offending key. That's a UX trade-off of the
    // `untagged` shape; the contract is just that the parse fails.
    let error = parse(content).expect_err("unknown source field must error");
    assert!(error.to_string().starts_with("Parse:"));
}

#[test]
fn parse_rejects_unknown_artifact_kind() {
    let content = r"
version: 1
sources:
    a:
        path: ./a
artifacts:
    a:
        plugins: [SomePlugin]
";
    let error = parse(content).expect_err("unknown artifact kind must error");
    let message = error.to_string().to_lowercase();
    assert!(
        message.contains("plugins") || message.contains("unknown"),
        "error must indicate the unknown artifact kind: {error}"
    );
}

#[test]
fn parse_rejects_wrong_schema_version() {
    let content = r"
version: 99
sources:
    a:
        path: ./a
";
    let error = parse(content).expect_err("version 99 must be rejected");
    assert!(
        error.to_string().contains("schema version 99"),
        "error must name the bad version: {error}"
    );
}

#[test]
fn parse_rejects_missing_version() {
    let content = r"
sources:
    a:
        path: ./a
";
    let error = parse(content).expect_err("missing version must be rejected");
    assert!(
        error.to_string().to_lowercase().contains("version"),
        "error must mention version: {error}"
    );
}

#[test]
fn parse_rejects_artifacts_without_matching_source() {
    let content = r"
version: 1
sources:
    a:
        path: ./a
artifacts:
    b:
        skills: [Something]
";
    let error = parse(content).expect_err("orphan artifacts entry must be rejected");
    let message = error.to_string();
    assert!(
        message.contains("'b'") && message.contains("no matching `sources`"),
        "error must explain the orphan binding: {error}"
    );
}

#[test]
fn parse_rejects_malformed_yaml() {
    let content = "version: 1\nsources:\n  a:\n   path: bad\n  - dangling";
    let error = parse(content).expect_err("malformed YAML must be rejected");
    assert!(
        error.to_string().contains(".forge"),
        "error must be tagged as .forge: {error}"
    );
}

#[test]
fn parse_accepts_empty_artifacts() {
    let content = r"
version: 1
sources:
    a:
        path: ./a
";
    let manifest = parse(content).unwrap();
    assert!(manifest.artifacts.is_empty());
}

#[test]
fn parse_accepts_artifact_list_with_only_one_kind() {
    let content = r"
version: 1
sources:
    a:
        path: ./a
artifacts:
    a:
        rules: [OnlyRule]
";
    let manifest = parse(content).unwrap();
    assert!(manifest.artifacts["a"].skills.is_empty());
    assert!(manifest.artifacts["a"].agents.is_empty());
    assert_eq!(manifest.artifacts["a"].rules, vec!["OnlyRule"]);
}
