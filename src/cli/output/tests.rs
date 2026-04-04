use super::*;
use commands::result::{ActionResult, DeployedFile, PrunedFile, SkipReason, SkippedFile};

#[test]
fn extract_content_kind_detects_agents() {
    assert_eq!(
        extract_content_kind("/home/.claude/agents/Dev.md"),
        "agents"
    );
}

#[test]
fn extract_content_kind_detects_skills() {
    assert_eq!(
        extract_content_kind("/home/.claude/skills/MySkill/SKILL.md"),
        "skills"
    );
}

#[test]
fn extract_content_kind_detects_rules() {
    assert_eq!(
        extract_content_kind("/home/.claude/rules/UseRTK.md"),
        "rules"
    );
}

#[test]
fn extract_content_kind_falls_back_to_files() {
    assert_eq!(extract_content_kind("/home/.claude/unknown.md"), "files");
}

#[test]
fn extract_relative_path_returns_last_three_segments() {
    let result = extract_relative_path("/home/user/.claude/rules/UseRTK.md");
    assert_eq!(result, ".claude/rules/UseRTK.md");
}

#[test]
fn extract_relative_path_short_path_returns_full() {
    assert_eq!(extract_relative_path("a/b"), "a/b");
}

#[test]
fn group_by_provider_groups_installed_files() {
    let result = ActionResult {
        installed: vec![
            DeployedFile {
                source: "build/claude/agents/Dev.md".to_string(),
                target: ".claude/agents/Dev.md".to_string(),
                provider: "claude".to_string(),
            },
            DeployedFile {
                source: "build/gemini/agents/dev.md".to_string(),
                target: ".gemini/agents/dev.md".to_string(),
                provider: "gemini".to_string(),
            },
        ],
        skipped: Vec::new(),
        pruned: Vec::new(),
        errors: Vec::new(),
    };

    let groups = group_by_provider(&result);
    assert!(groups.contains_key("claude"));
    assert!(groups.contains_key("gemini"));
    assert_eq!(groups.len(), 2);
}

#[test]
fn group_by_provider_includes_skipped_files() {
    let result = ActionResult {
        installed: Vec::new(),
        skipped: vec![SkippedFile {
            target: ".claude/rules/UseRTK.md".to_string(),
            provider: "claude".to_string(),
            reason: SkipReason::Unchanged,
        }],
        pruned: Vec::new(),
        errors: Vec::new(),
    };

    let groups = group_by_provider(&result);
    assert_eq!(groups["claude"].skips.len(), 1);
}

#[test]
fn group_by_provider_includes_pruned_files() {
    let result = ActionResult {
        installed: Vec::new(),
        skipped: Vec::new(),
        pruned: vec![PrunedFile {
            target: ".claude/rules/Old.md".to_string(),
            provider: "claude".to_string(),
        }],
        errors: Vec::new(),
    };

    let groups = group_by_provider(&result);
    assert_eq!(groups["claude"].pruned.len(), 1);
}

#[test]
fn group_by_provider_empty_result_is_empty() {
    let result = ActionResult::new();
    let groups = group_by_provider(&result);
    assert!(groups.is_empty());
}
