use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaletteCommand {
    Refresh,
    Quit,
    Find(String),
    GoTo(String),
    Sort(String),
    Filter(String),
    Empty,
    Unknown(String),
}

#[derive(Debug, Clone, Default)]
pub struct Palette {
    input: String,
    open: bool,
}

impl Palette {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn open(&mut self) {
        self.open = true;
        self.input.clear();
    }

    pub fn close(&mut self) {
        self.open = false;
        self.input.clear();
    }

    pub fn take_command(&mut self) -> PaletteCommand {
        let command = Self::parse_command(&self.input);
        self.close();
        command
    }

    #[must_use]
    pub fn parse_command(input: &str) -> PaletteCommand {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return PaletteCommand::Empty;
        }
        let mut parts = trimmed.splitn(2, char::is_whitespace);
        let verb = parts.next().unwrap_or_default();
        let rest = parts.next().unwrap_or_default().trim();
        match verb {
            "r" | "refresh" => PaletteCommand::Refresh,
            "q" | "quit" => PaletteCommand::Quit,
            "find" => PaletteCommand::Find(rest.to_string()),
            "sort" => PaletteCommand::Sort(rest.to_string()),
            "filter" => PaletteCommand::Filter(rest.to_string()),
            "overview" | "skills" | "agents" | "rules" | "repos" | "repositories" | "adrs"
            | "provenance" | "variants" | "search" | "settings" | "hooks" | "config"
            | "schemas" => PaletteCommand::GoTo(verb.to_string()),
            other => PaletteCommand::Unknown(other.to_string()),
        }
    }

    pub fn display_text(&self, error: Option<&str>) -> String {
        if let Some(error) = error {
            format!("error: {error}")
        } else {
            let prefix = if self.open { ":" } else { "" };
            format!("{prefix}{}", self.input)
        }
    }

    pub fn on_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Backspace => {
                self.input.pop();
                true
            }
            KeyCode::Char(character)
                if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
            {
                self.input.push(character);
                true
            }
            // TODO: numbered selection and 3-level tab-completion for v2.
            _ => false,
        }
    }
}
