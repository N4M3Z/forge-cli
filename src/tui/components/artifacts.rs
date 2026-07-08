use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
};

use commands::view::{ArtifactView, DashboardView};

use super::{Component, Outcome};

#[derive(Debug, Clone)]
pub struct ArtifactsPane {
    view: DashboardView,
    selected: usize,
    focused: bool,
}

impl ArtifactsPane {
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

    #[must_use]
    pub fn selected_artifact(&self) -> Option<&ArtifactView> {
        self.artifact_refs()
            .get(self.selected)
            .and_then(|(module_index, artifact_index)| {
                self.view
                    .modules
                    .get(*module_index)
                    .and_then(|module| module.artifacts.get(*artifact_index))
            })
    }

    fn artifact_refs(&self) -> Vec<(usize, usize)> {
        self.view
            .modules
            .iter()
            .enumerate()
            .flat_map(|(module_index, module)| {
                module
                    .artifacts
                    .iter()
                    .enumerate()
                    .map(move |(artifact_index, _)| (module_index, artifact_index))
            })
            .collect()
    }

    fn clamp_selection(&mut self) {
        let len = self.artifact_refs().len();
        if len == 0 {
            self.selected = 0;
        } else if self.selected >= len {
            self.selected = len - 1;
        }
    }

    fn move_down(&mut self) {
        let len = self.artifact_refs().len();
        if self.selected + 1 < len {
            self.selected += 1;
        }
    }

    fn move_up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    fn border_style(&self) -> Style {
        if self.focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        }
    }
}

impl Component for ArtifactsPane {
    fn render(&self, frame: &mut Frame<'_>, area: Rect) {
        let outer = Block::default()
            .title(" Artifacts ")
            .borders(Borders::ALL)
            .border_style(self.border_style());
        let inner = outer.inner(area);
        frame.render_widget(outer, area);

        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
            .split(inner);

        let mut selected_seen = 0usize;
        let mut items = Vec::new();
        for module in &self.view.modules {
            items.push(ListItem::new(Line::from(Span::styled(
                module.name.clone(),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ))));
            for (kind, artifacts) in module.artifacts_by_kind() {
                for artifact in artifacts {
                    let is_selected = selected_seen == self.selected;
                    selected_seen += 1;
                    let style = if is_selected {
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::Cyan)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    };
                    items.push(ListItem::new(Line::from(vec![
                        Span::raw("  "),
                        Span::styled(artifact.name.clone(), style),
                        Span::styled(format!("  {kind}"), style.fg(Color::Gray)),
                    ])));
                }
            }
        }
        if items.is_empty() {
            items.push(ListItem::new("no artifacts"));
        }

        frame.render_widget(List::new(items), chunks[0]);

        let detail = match self.selected_artifact() {
            Some(artifact) => {
                let body = if artifact.content_body.is_empty() {
                    artifact.content_preview.as_str()
                } else {
                    artifact.content_body.as_str()
                };
                Text::from(vec![
                    Line::from(Span::styled(
                        artifact.name.clone(),
                        Style::default().add_modifier(Modifier::BOLD),
                    )),
                    Line::from(format!("kind: {}", artifact.kind)),
                    Line::from(format!("module: {}", artifact.module)),
                    Line::from(format!("path: {}", artifact.relative_path)),
                    Line::from(Span::styled(
                        "↵ open full preview",
                        Style::default().fg(Color::Cyan),
                    )),
                    Line::from(""),
                    Line::from(artifact.description.clone()),
                    Line::from(""),
                    Line::from(body.to_string()),
                ])
            }
            None => Text::from("no artifact selected"),
        };
        frame.render_widget(Paragraph::new(detail).wrap(Wrap { trim: false }), chunks[1]);
    }

    fn on_key(&mut self, key: KeyEvent) -> Outcome {
        match key.code {
            KeyCode::Down | KeyCode::Char('j') => {
                self.move_down();
                Outcome::Handled
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.move_up();
                Outcome::Handled
            }
            _ => Outcome::Ignored,
        }
    }
}
