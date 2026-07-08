use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
};

use super::{Component, Outcome};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaletteCommand {
    Refresh,
    Quit,
    Find(String),
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
            other => PaletteCommand::Unknown(other.to_string()),
        }
    }

    pub fn render_with_error(&self, frame: &mut Frame<'_>, area: Rect, error: Option<&str>) {
        let border_style = if self.open {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        let text = if let Some(error) = error {
            format!("error: {error}")
        } else {
            let prefix = if self.open { ":" } else { "" };
            format!("{prefix}{}", self.input)
        };
        frame.render_widget(
            Paragraph::new(text).block(
                Block::default()
                    .title(" Command ")
                    .borders(Borders::ALL)
                    .border_style(border_style),
            ),
            area,
        );
    }
}

impl Component for Palette {
    fn render(&self, frame: &mut Frame<'_>, area: Rect) {
        self.render_with_error(frame, area, None);
    }

    fn on_key(&mut self, key: KeyEvent) -> Outcome {
        match key.code {
            KeyCode::Backspace => {
                self.input.pop();
                Outcome::Handled
            }
            KeyCode::Char(character)
                if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
            {
                self.input.push(character);
                Outcome::Handled
            }
            // TODO: numbered selection and 3-level tab-completion for v2.
            _ => Outcome::Ignored,
        }
    }
}
