use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
};

use commands::view::DashboardView;

use super::{Component, Outcome};

#[derive(Debug, Clone)]
pub struct SourcesPane {
    view: DashboardView,
    watched_locations: Vec<PathBuf>,
    query: String,
    selected: usize,
    focused: bool,
}

impl SourcesPane {
    #[must_use]
    pub fn new(view: DashboardView, watched_locations: Vec<PathBuf>) -> Self {
        Self {
            view,
            watched_locations,
            query: String::new(),
            selected: 0,
            focused: false,
        }
    }

    pub fn set_view(&mut self, view: DashboardView, watched_locations: Vec<PathBuf>) {
        self.view = view;
        self.watched_locations = watched_locations;
        self.clamp_selection();
    }

    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    pub fn set_query(&mut self, query: impl Into<String>) {
        self.query = query.into();
        self.selected = 0;
        self.clamp_selection();
    }

    fn filtered_matches(&self) -> Vec<String> {
        let query = self.query.to_lowercase();
        self.view
            .all_artifacts()
            .into_iter()
            .filter(|(artifact, _module)| query.is_empty() || artifact.matches_query(&query))
            .map(|(artifact, module)| format!("{module} / {}", artifact.name))
            .collect()
    }

    fn clamp_selection(&mut self) {
        let len = self.filtered_matches().len();
        if len == 0 {
            self.selected = 0;
        } else if self.selected >= len {
            self.selected = len - 1;
        }
    }

    fn border_style(&self) -> Style {
        if self.focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        }
    }
}

impl Component for SourcesPane {
    fn render(&self, frame: &mut Frame<'_>, area: Rect) {
        let block = Block::default()
            .title(" Sources + watch + find ")
            .borders(Borders::ALL)
            .border_style(self.border_style());
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(5),
                Constraint::Length(3),
                Constraint::Min(1),
            ])
            .split(inner);

        let mut watch_lines = Vec::new();
        for module in self.view.target_modules() {
            watch_lines.push(Line::from(format!("target: {}", module.name)));
        }
        for path in &self.watched_locations {
            watch_lines.push(Line::from(format!("watch: {}", path.display())));
        }
        if watch_lines.is_empty() {
            watch_lines.push(Line::from("no watched sources"));
        }
        frame.render_widget(
            Paragraph::new(Text::from(watch_lines)).wrap(Wrap { trim: false }),
            chunks[0],
        );

        frame.render_widget(
            Paragraph::new(format!("find: {}", self.query))
                .block(Block::default().borders(Borders::ALL)),
            chunks[1],
        );

        let matches = self.filtered_matches();
        let items: Vec<ListItem<'_>> = if matches.is_empty() {
            vec![ListItem::new("no matches")]
        } else {
            matches
                .into_iter()
                .enumerate()
                .map(|(index, label)| {
                    let style = if index == self.selected {
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::Cyan)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    };
                    ListItem::new(Line::from(Span::styled(label, style)))
                })
                .collect()
        };
        frame.render_widget(List::new(items), chunks[2]);
    }

    fn on_key(&mut self, key: KeyEvent) -> Outcome {
        match key.code {
            KeyCode::Down | KeyCode::Char('j') => {
                let len = self.filtered_matches().len();
                if self.selected + 1 < len {
                    self.selected += 1;
                }
                Outcome::Handled
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.selected = self.selected.saturating_sub(1);
                Outcome::Handled
            }
            KeyCode::Backspace => {
                self.query.pop();
                self.clamp_selection();
                Outcome::Handled
            }
            KeyCode::Char(character)
                if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
            {
                self.query.push(character);
                self.selected = 0;
                self.clamp_selection();
                Outcome::Handled
            }
            _ => Outcome::Ignored,
        }
    }
}
