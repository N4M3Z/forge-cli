use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use commands::view::ArtifactView;

/// Approximate the number of rows the body occupies when word-wrapped to
/// `width`, so the scroll offset can be clamped to a reachable bottom. Counts
/// each source line as at least one row plus a row per `width` characters
/// beyond the first; wide glyphs are treated as one column (close enough to
/// keep the last line reachable without pulling in a unicode-width dependency).
fn wrapped_line_count(body: &str, width: u16) -> u16 {
    if width == 0 {
        return u16::try_from(body.lines().count()).unwrap_or(u16::MAX);
    }
    let width = usize::from(width);
    let mut rows: usize = 0;
    for line in body.lines() {
        let chars = line.chars().count().max(1);
        rows = rows.saturating_add(chars.div_ceil(width));
    }
    u16::try_from(rows).unwrap_or(u16::MAX)
}

/// Full-window scrollable view of a single artifact's body. Opened from the
/// artifacts pane with Enter, so a skill's full content is readable instead of
/// clipped into a quarter-pane detail column.
#[derive(Debug, Clone)]
pub struct ArtifactPreview {
    title: String,
    body: String,
    scroll: u16,
}

impl ArtifactPreview {
    #[must_use]
    pub fn from_artifact(artifact: &ArtifactView) -> Self {
        let body = if artifact.content_body.is_empty() {
            artifact.content_preview.clone()
        } else {
            artifact.content_body.clone()
        };
        let title = format!(
            " {}  ·  {}  ·  {} ",
            artifact.name, artifact.kind, artifact.relative_path
        );
        Self {
            title,
            body,
            scroll: 0,
        }
    }

    pub fn scroll_down(&mut self, amount: u16) {
        self.scroll = self.scroll.saturating_add(amount);
    }

    pub fn scroll_up(&mut self, amount: u16) {
        self.scroll = self.scroll.saturating_sub(amount);
    }

    pub fn scroll_to_top(&mut self) {
        self.scroll = 0;
    }

    pub fn scroll_to_bottom(&mut self) {
        self.scroll = u16::MAX;
    }

    /// Render takes `&mut self` so the scroll offset can be clamped against the
    /// real wrapped line count at the current width — the only place the true
    /// content height is known.
    pub fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let block = Block::default()
            .title(self.title.as_str())
            .title_bottom(Line::from(Span::styled(
                " j/k · ␣/b page · g/G ends · Esc close ",
                Style::default().fg(Color::DarkGray),
            )))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));

        let inner_width = area.width.saturating_sub(2);
        let inner_height = area.height.saturating_sub(2);

        let total = wrapped_line_count(&self.body, inner_width);
        let max_scroll = total.saturating_sub(inner_height);
        if self.scroll > max_scroll {
            self.scroll = max_scroll;
        }

        let paragraph = Paragraph::new(self.body.as_str()).wrap(Wrap { trim: false });

        frame.render_widget(
            paragraph
                .block(block.title_top(Line::from(Span::styled(
                    format!(" {}/{} ", self.scroll.saturating_add(1), total.max(1)),
                    Style::default().fg(Color::Gray).add_modifier(Modifier::DIM),
                ))))
                .scroll((self.scroll, 0)),
            area,
        );
    }
}
