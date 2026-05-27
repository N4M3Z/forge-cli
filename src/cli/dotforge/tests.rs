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
    let Source::Local { path } = &manifest.sources["forge-core"] else {
        panic!("expected Local source for forge-core");
    };
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
fn parse_accepts_git_source_with_https_url_and_full_sha() {
    let content = r"
version: 1
sources:
    forge-core:
        git: https://github.com/N4M3Z/forge-core
        ref: 0d83a3b9f4e2c1a8b7d6e5f4c3b2a1098765432d
";
    let manifest = parse(content).expect("git source manifest must parse");
    let Source::Git { git, commit } = &manifest.sources["forge-core"] else {
        panic!("expected Git source for forge-core");
    };
    assert_eq!(git, "https://github.com/N4M3Z/forge-core");
    assert_eq!(commit, "0d83a3b9f4e2c1a8b7d6e5f4c3b2a1098765432d");
}

#[test]
fn parse_rejects_git_source_with_http_url() {
    let content = r"
version: 1
sources:
    bad:
        git: http://github.com/N4M3Z/forge-core
        ref: 0d83a3b9f4e2c1a8b7d6e5f4c3b2a1098765432d
";
    let error = parse(content).expect_err("http:// must be rejected");
    assert!(
        error.to_string().contains("https"),
        "error must call out the https requirement: {error}"
    );
}

#[test]
fn parse_rejects_git_source_with_ssh_shorthand() {
    let content = r"
version: 1
sources:
    bad:
        git: git@github.com:N4M3Z/forge-core.git
        ref: 0d83a3b9f4e2c1a8b7d6e5f4c3b2a1098765432d
";
    let error = parse(content).expect_err("git@host: shorthand must be rejected");
    assert!(
        error.to_string().contains("https"),
        "error must call out the https requirement: {error}"
    );
}

#[test]
fn parse_rejects_git_source_with_userinfo_in_url() {
    let content = r"
version: 1
sources:
    bad:
        git: https://attacker:pass@github.com/N4M3Z/forge-core
        ref: 0d83a3b9f4e2c1a8b7d6e5f4c3b2a1098765432d
";
    let error = parse(content).expect_err("userinfo in URL must be rejected");
    assert!(
        error.to_string().to_lowercase().contains("user@"),
        "error must call out the userinfo ban: {error}"
    );
}

#[test]
fn parse_rejects_git_source_with_branch_name_as_ref() {
    let content = r"
version: 1
sources:
    bad:
        git: https://github.com/N4M3Z/forge-core
        ref: main
";
    let error = parse(content).expect_err("branch name must be rejected");
    let message = error.to_string();
    assert!(
        message.contains("40-char") || message.contains("commit SHA"),
        "error must require a full commit SHA: {error}"
    );
}

#[test]
fn parse_rejects_git_source_with_short_sha() {
    let content = r"
version: 1
sources:
    bad:
        git: https://github.com/N4M3Z/forge-core
        ref: 0d83a3b9
";
    let error = parse(content).expect_err("short SHA must be rejected");
    assert!(
        error.to_string().contains("40-char"),
        "error must require a 40-char SHA: {error}"
    );
}

#[test]
fn parse_rejects_git_source_with_uppercase_sha() {
    let content = r"
version: 1
sources:
    bad:
        git: https://github.com/N4M3Z/forge-core
        ref: 0D83A3B9F4E2C1A8B7D6E5F4C3B2A1098765432D
";
    let error = parse(content).expect_err("uppercase SHA must be rejected");
    assert!(
        error.to_string().contains("lowercase"),
        "error must require lowercase hex: {error}"
    );
}

#[test]
fn parse_rejects_git_source_with_non_hex_sha() {
    let content = r"
version: 1
sources:
    bad:
        git: https://github.com/N4M3Z/forge-core
        ref: zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz
";
    let error = parse(content).expect_err("non-hex SHA must be rejected");
    assert!(
        error.to_string().contains("hex"),
        "error must require hex chars: {error}"
    );
}

#[test]
fn parse_rejects_git_source_missing_ref() {
    let content = r"
version: 1
sources:
    bad:
        git: https://github.com/N4M3Z/forge-core
";
    let error = parse(content).expect_err("git source missing ref must be rejected");
    assert!(
        error.to_string().starts_with("Parse:"),
        "error must be a parse error: {error}"
    );
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
