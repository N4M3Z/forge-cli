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
