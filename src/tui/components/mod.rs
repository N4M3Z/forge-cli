pub mod artifacts;
pub mod palette;
pub mod preview;
pub mod projects;
pub mod provenance;
pub mod sources;

use crossterm::event::KeyEvent;
use ratatui::{Frame, layout::Rect};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Handled,
    Ignored,
}

pub trait Component {
    fn render(&self, frame: &mut Frame<'_>, area: Rect);
    fn on_key(&mut self, key: KeyEvent) -> Outcome;
}
