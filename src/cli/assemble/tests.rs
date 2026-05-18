use super::*;
use std::collections::HashMap;

fn make_models() -> HashMap<String, Vec<String>> {
    let mut models = HashMap::new();
    models.insert(
        "claude".to_string(),
        vec![
            "claude-opus-4-6".to_string(),
            "claude-sonnet-4-6".to_string(),
        ],
    );
    models.insert("codex".to_string(), vec!["o4-mini".to_string()]);
    models.insert(
        "opencode".to_string(),
        vec!["claude-sonnet-4-6".to_string()],
    );
    models
}

#[test]
fn direct_provider_name_matches() {
    let models = make_models();
    assert!(qualifier_matches_provider("claude", "claude", &models));
    assert!(qualifier_matches_provider("codex", "codex", &models));
}

#[test]
fn model_tier_matches_provider_with_that_model() {
    let models = make_models();
    assert!(qualifier_matches_provider("sonnet", "claude", &models));
    assert!(qualifier_matches_provider("opus", "claude", &models));
}

#[test]
fn model_tier_matches_across_providers() {
    let models = make_models();
    assert!(qualifier_matches_provider("sonnet", "opencode", &models));
}

#[test]
fn model_tier_does_not_match_unrelated_provider() {
    let models = make_models();
    assert!(!qualifier_matches_provider("sonnet", "codex", &models));
    assert!(!qualifier_matches_provider("opus", "codex", &models));
}

#[test]
fn unknown_qualifier_does_not_match() {
    let models = make_models();
    assert!(!qualifier_matches_provider("gpt5", "claude", &models));
}

#[test]
fn provider_not_in_models_only_matches_by_name() {
    let models = make_models();
    assert!(qualifier_matches_provider("gemini", "gemini", &models));
    assert!(!qualifier_matches_provider("sonnet", "gemini", &models));
}

#[test]
fn kebab_case_path_converts_simple_filename() {
    assert_eq!(apply_kebab_case_to_path("GameMaster.md"), "game-master.md");
}

#[test]
fn kebab_case_path_converts_directory_and_filename() {
    assert_eq!(
        apply_kebab_case_to_path("SceneReview/SKILL.md"),
        "scene-review/skill.md"
    );
}

#[test]
fn kebab_case_path_preserves_already_lowercase() {
    assert_eq!(apply_kebab_case_to_path("readme.md"), "readme.md");
}

#[test]
fn assemble_source_maps_agent_model_and_effort_tiers() {
    let source = sources::SourceFile {
        kind: commands::provider::ContentKind::Agents,
        relative_path: "agents/TestAgent.md".to_string(),
        full_path: "/tmp/TestAgent.md".to_string(),
        qualifier: None,
        passthrough: false,
        targets: None,
        content: "---\nname: TestAgent\ndescription: test\nmodel: strong\n---\n\nBody.\n"
            .to_string(),
    };
    let mut model_tiers = HashMap::new();
    model_tiers.insert("strong".to_string(), vec!["o3".to_string()]);
    let mut effort_tiers = HashMap::new();
    effort_tiers.insert("strong".to_string(), "medium".to_string());

    let result = pipeline::assemble_source(
        &source,
        std::path::Path::new("/tmp"),
        "codex",
        &[
            "name".to_string(),
            "description".to_string(),
            "model".to_string(),
            "effort".to_string(),
        ],
        &model_tiers,
        &effort_tiers,
        false,
    )
    .unwrap();

    assert!(result.contains("model: o3"));
    assert!(result.contains("effort: medium"));
}

#[test]
fn assemble_source_maps_all_codex_tiers() {
    let mut model_tiers = HashMap::new();
    model_tiers.insert("strong".to_string(), vec!["gpt-5.5".to_string()]);
    model_tiers.insert("fast".to_string(), vec!["gpt-5.4".to_string()]);
    model_tiers.insert("light".to_string(), vec!["gpt-5.3-codex".to_string()]);

    let mut effort_tiers = HashMap::new();
    effort_tiers.insert("strong".to_string(), "medium".to_string());
    effort_tiers.insert("fast".to_string(), "low".to_string());
    effort_tiers.insert("light".to_string(), "low".to_string());

    let cases = [
        ("strong", Some("gpt-5.5"), Some("medium")),
        ("fast", Some("gpt-5.4"), Some("low")),
        ("light", Some("gpt-5.3-codex"), Some("low")),
        ("unmapped", None, None),
    ];

    for (source_model, expected_model, expected_effort) in cases {
        let source = sources::SourceFile {
            kind: commands::provider::ContentKind::Agents,
            relative_path: "agents/TestAgent.md".to_string(),
            full_path: "/tmp/TestAgent.md".to_string(),
            qualifier: None,
            passthrough: false,
            targets: None,
            content: format!(
                "---\nname: TestAgent\ndescription: test\nmodel: {source_model}\n---\n\nBody.\n"
            ),
        };

        let result = pipeline::assemble_source(
            &source,
            std::path::Path::new("/tmp"),
            "codex",
            &[
                "name".to_string(),
                "description".to_string(),
                "model".to_string(),
                "effort".to_string(),
            ],
            &model_tiers,
            &effort_tiers,
            false,
        )
        .unwrap();

        let model_line = match expected_model {
            Some(model) => format!("model: {model}"),
            None => format!("model: {source_model}"),
        };
        assert!(
            result.contains(&model_line),
            "tier {source_model}: expected `{model_line}`, got:\n{result}"
        );

        match expected_effort {
            Some(effort) => assert!(
                result.contains(&format!("effort: {effort}")),
                "tier {source_model}: expected effort {effort}, got:\n{result}"
            ),
            None => assert!(
                !result.contains("effort:"),
                "tier {source_model}: expected no effort, got:\n{result}"
            ),
        }
    }
}

#[test]
fn assemble_source_keeps_explicit_effort_over_tier_effort() {
    let source = sources::SourceFile {
        kind: commands::provider::ContentKind::Agents,
        relative_path: "agents/TestAgent.md".to_string(),
        full_path: "/tmp/TestAgent.md".to_string(),
        qualifier: None,
        passthrough: false,
        targets: None,
        content:
            "---\nname: TestAgent\ndescription: test\nmodel: strong\neffort: high\n---\n\nBody.\n"
                .to_string(),
    };
    let mut model_tiers = HashMap::new();
    model_tiers.insert("strong".to_string(), vec!["o3".to_string()]);
    let mut effort_tiers = HashMap::new();
    effort_tiers.insert("strong".to_string(), "medium".to_string());

    let result = pipeline::assemble_source(
        &source,
        std::path::Path::new("/tmp"),
        "codex",
        &[
            "name".to_string(),
            "description".to_string(),
            "model".to_string(),
            "effort".to_string(),
        ],
        &model_tiers,
        &effort_tiers,
        false,
    )
    .unwrap();

    assert!(result.contains("model: o3"));
    assert!(result.contains("effort: high"));
    assert!(!result.contains("effort: medium"));
}
