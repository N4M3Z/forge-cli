use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ErrorKind {
    Parse,
    Config,
    Io,
    Deploy,
    Validate,
}

#[derive(Debug)]
pub struct Error {
    kind: ErrorKind,
    message: String,
}

impl Error {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:?}: {}", self.kind, self.message)
    }
}

impl std::error::Error for Error {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_preserves_kind() {
        let error = Error::new(ErrorKind::Parse, "bad yaml");
        assert_eq!(error.kind(), ErrorKind::Parse);
    }

    #[test]
    fn error_preserves_message() {
        let error = Error::new(ErrorKind::Io, "file not found");
        assert_eq!(error.message(), "file not found");
    }

    #[test]
    fn error_accepts_string_message() {
        let error = Error::new(ErrorKind::Config, String::from("invalid config"));
        assert_eq!(error.message(), "invalid config");
    }

    #[test]
    fn display_includes_kind_and_message() {
        let error = Error::new(ErrorKind::Deploy, "target missing");
        let display = format!("{error}");
        assert!(display.contains("Deploy"));
        assert!(display.contains("target missing"));
    }

    #[test]
    fn error_kind_equality() {
        assert_eq!(ErrorKind::Parse, ErrorKind::Parse);
        assert_ne!(ErrorKind::Parse, ErrorKind::Io);
    }

    #[test]
    fn error_implements_std_error() {
        let error = Error::new(ErrorKind::Validate, "schema mismatch");
        let std_error: &dyn std::error::Error = &error;
        assert!(std_error.to_string().contains("schema mismatch"));
    }
}
