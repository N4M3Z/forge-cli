use serde::Serialize;

#[derive(Debug, Default, Serialize)]
pub struct ActionResult {
    pub installed: Vec<DeployedFile>,
    pub skipped: Vec<SkippedFile>,
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

#[derive(Debug, Clone, Serialize)]
pub enum SkipReason {
    UserModified,
    TargetMismatch,
    Unchanged,
}
