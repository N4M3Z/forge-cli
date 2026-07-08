use crossterm::event::KeyEvent;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use commands::view::{ArtifactView, DashboardView};

use super::{Component, Outcome};

#[derive(Debug, Clone)]
pub struct ProvenancePane {
    view: DashboardView,
    selected_artifact: Option<ArtifactView>,
    focused: bool,
}

impl ProvenancePane {
    #[must_use]
    pub fn new(view: DashboardView) -> Self {
        Self {
            view,
            selected_artifact: None,
            focused: false,
        }
    }

    pub fn set_view(&mut self, view: DashboardView) {
        self.view = view;
    }

    pub fn set_selected_artifact(&mut self, artifact: Option<ArtifactView>) {
        self.selected_artifact = artifact;
    }

    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    fn border_style(&self) -> Style {
        if self.focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        }
    }
}

impl Component for ProvenancePane {
    fn render(&self, frame: &mut Frame<'_>, area: Rect) {
        let block = Block::default()
            .title(" Provenance + integrity ")
            .borders(Borders::ALL)
            .border_style(self.border_style());
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let Some(artifact) = &self.selected_artifact else {
            frame.render_widget(Paragraph::new("no artifact selected"), inner);
            return;
        };

        let mut lines = vec![
            Line::from(vec![
                Span::styled("status: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(artifact.overall_status()),
            ]),
            Line::from(vec![
                Span::styled("staleness: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(artifact.staleness_label()),
            ]),
        ];

        if !artifact.broken_refs.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "broken references",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            )));
            for broken_ref in &artifact.broken_refs {
                lines.push(Line::from(format!("  {broken_ref}")));
            }
        }

        if !artifact.sidecar_warning.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled("sidecar: ", Style::default().fg(Color::Yellow)),
                Span::raw(artifact.sidecar_warning.clone()),
            ]));
        }

        if !artifact.providers.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "providers",
                Style::default().add_modifier(Modifier::BOLD),
            )));
            for (provider, status) in &artifact.providers {
                lines.push(Line::from(format!(
                    "  {provider} -> {}",
                    status.status.label()
                )));
            }
        }

        if let Some(adoption) = &artifact.adoption {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "adoption",
                Style::default().add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(format!("  source: {}", adoption.source_label)));
            lines.push(Line::from(format!("  sha: {}", adoption.source_sha)));
            if !adoption.transforms.is_empty() {
                lines.push(Line::from(format!(
                    "  transforms: {}",
                    adoption.transforms.join(", ")
                )));
            }
        }

        let provenance_rows: Vec<_> = self
            .view
            .provenance
            .iter()
            .flat_map(|provenance| provenance.artifacts.iter())
            .filter(|row| row.source_path.ends_with(&artifact.relative_path))
            .collect();
        if !provenance_rows.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "deployments",
                Style::default().add_modifier(Modifier::BOLD),
            )));
            for row in provenance_rows {
                let verified = if row.verified { "verified" } else { "mismatch" };
                lines.push(Line::from(format!(
                    "  {} -> {} [{}]",
                    row.source_path, row.deployed_path, row.harness
                )));
                lines.push(Line::from(format!(
                    "    {verified}; deployed {}; expected {}",
                    row.deployed_sha, row.expected_sha
                )));
            }
        }

        frame.render_widget(
            Paragraph::new(Text::from(lines)).wrap(Wrap { trim: false }),
            inner,
        );
    }

    fn on_key(&mut self, _key: KeyEvent) -> Outcome {
        Outcome::Ignored
    }
}
