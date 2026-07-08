use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use commands::view::DashboardView;

use super::{Component, Outcome};

#[derive(Debug, Clone)]
pub struct ProjectsPane {
    view: DashboardView,
    selected: usize,
    focused: bool,
}

impl ProjectsPane {
    #[must_use]
    pub fn new(view: DashboardView) -> Self {
        Self {
            view,
            selected: 0,
            focused: false,
        }
    }

    pub fn set_view(&mut self, view: DashboardView) {
        self.view = view;
        self.clamp_selection();
    }

    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    fn clamp_selection(&mut self) {
        if self.view.modules.is_empty() {
            self.selected = 0;
        } else if self.selected >= self.view.modules.len() {
            self.selected = self.view.modules.len() - 1;
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

impl Component for ProjectsPane {
    fn render(&self, frame: &mut Frame<'_>, area: Rect) {
        let block = Block::default()
            .title(" Projects + ontology ")
            .borders(Borders::ALL)
            .border_style(self.border_style());
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let mut lines = vec![
            Line::from(Span::styled(
                "no ontology configured",
                Style::default().fg(Color::Yellow),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "modules",
                Style::default().add_modifier(Modifier::BOLD),
            )),
        ];

        if self.view.modules.is_empty() {
            lines.push(Line::from("  no modules"));
        } else {
            for (index, module) in self.view.modules.iter().enumerate() {
                let style = if index == self.selected {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                lines.push(Line::from(Span::styled(
                    format!("  {}", module.name),
                    style,
                )));
            }
        }

        frame.render_widget(
            Paragraph::new(Text::from(lines)).wrap(Wrap { trim: false }),
            inner,
        );
    }

    fn on_key(&mut self, key: KeyEvent) -> Outcome {
        match key.code {
            KeyCode::Down | KeyCode::Char('j') => {
                if self.selected + 1 < self.view.modules.len() {
                    self.selected += 1;
                }
                Outcome::Handled
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.selected = self.selected.saturating_sub(1);
                Outcome::Handled
            }
            _ => Outcome::Ignored,
        }
    }
}
