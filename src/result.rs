use serde::Serialize;

#[derive(Debug, Default, Serialize)]
pub struct ActionResult {
    pub installed: Vec<DeployedFile>,
    pub skipped: Vec<SkippedFile>,
    pub pruned: Vec<PrunedFile>,
    pub errors: Vec<String>,
}

impl ActionResult {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }
}

#[derive(Debug, Serialize)]
pub struct DeployedFile {
    pub source: String,
    pub target: String,
    pub provider: String,
}

#[derive(Debug, Serialize)]
pub struct SkippedFile {
    pub target: String,
    pub provider: String,
    pub reason: SkipReason,
}

#[derive(Debug, Serialize)]
pub struct PrunedFile {
    pub target: String,
    pub provider: String,
}

#[derive(Debug, Clone, Serialize)]
pub enum SkipReason {
    UserModified,
    TargetMismatch,
    Unchanged,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_result_is_empty() {
        let result = ActionResult::new();
        assert!(result.installed.is_empty());
        assert!(result.skipped.is_empty());
        assert!(result.pruned.is_empty());
        assert!(result.errors.is_empty());
    }

    #[test]
    fn has_errors_returns_false_when_empty() {
        let result = ActionResult::new();
        assert!(!result.has_errors());
    }

    #[test]
    fn has_errors_returns_true_with_errors() {
        let mut result = ActionResult::new();
        result.errors.push("something broke".to_string());
        assert!(result.has_errors());
    }

    #[test]
    fn default_matches_new() {
        let from_new = ActionResult::new();
        let from_default = ActionResult::default();
        assert_eq!(from_new.installed.len(), from_default.installed.len());
        assert_eq!(from_new.errors.len(), from_default.errors.len());
    }

    #[test]
    fn serializes_to_json() {
        let mut result = ActionResult::new();
        result.installed.push(DeployedFile {
            source: "build/claude/rules/Test.md".to_string(),
            target: ".claude/rules/Test.md".to_string(),
            provider: "claude".to_string(),
        });
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("Test.md"));
        assert!(json.contains("claude"));
    }
}
