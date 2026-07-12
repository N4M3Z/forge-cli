use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use commands::view::ArtifactView;

use super::super::app::DetailTab;

/// Display rows the lines occupy when word-wrapped to `width`. Counts each
/// line as at least one row plus one per full width beyond it; wide glyphs
/// count as one column, close enough to keep the last line reachable.
pub(in super::super) fn wrapped_rows(lines: &[Line<'_>], width: u16) -> usize {
    if width == 0 {
        return lines.len();
    }
    let width = usize::from(width);
    lines
        .iter()
        .map(|line| line.width().max(1).div_ceil(width))
        .sum()
}

/// Rendered lines for one (tab, width) combination, rebuilt only when either
/// changes so scrolling a large file stays cheap.
#[derive(Debug, Clone)]
struct PreviewPane {
    tab: DetailTab,
    width: u16,
    lines: Vec<Line<'static>>,
    windowed: bool,
}

/// Full-window scrollable view of the selected artifact. Opened from the
/// detail pane with Enter; shows the same rich tabs as the pane (digits
/// switch), so zooming never loses highlighting or layout.
#[derive(Debug, Clone)]
pub struct ArtifactPreview {
    artifact: ArtifactView,
    scroll: u16,
    pane: Option<PreviewPane>,
}

impl ArtifactPreview {
    #[must_use]
    pub fn from_artifact(artifact: &ArtifactView) -> Self {
        Self {
            artifact: artifact.clone(),
            scroll: 0,
            pane: None,
        }
    }

    #[must_use]
    pub fn artifact(&self) -> &ArtifactView {
        &self.artifact
    }

    #[must_use]
    pub fn scroll(&self) -> u16 {
        self.scroll
    }

    #[must_use]
    pub fn needs_rebuild(&self, tab: DetailTab, width: u16) -> bool {
        self.pane
            .as_ref()
            .is_none_or(|pane| pane.tab != tab || pane.width != width)
    }

    pub fn set_lines(
        &mut self,
        tab: DetailTab,
        width: u16,
        lines: Vec<Line<'static>>,
        windowed: bool,
    ) {
        if self.pane.as_ref().is_some_and(|pane| pane.tab != tab) {
            self.scroll = 0;
        }
        self.pane = Some(PreviewPane {
            tab,
            width,
            lines,
            windowed,
        });
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

    pub fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let tab_label = self
            .pane
            .as_ref()
            .map_or("Preview", |pane| pane.tab.label());
        let title = format!(
            " {} · {} · {} ",
            self.artifact.name, tab_label, self.artifact.relative_path
        );
        let block = Block::default()
            .title(title)
            .title_bottom(Line::from(Span::styled(
                " 1-7 tabs · j/k · ␣/b page · g/G ends · Esc close ",
                Style::default().fg(Color::DarkGray),
            )))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));
        let inner = block.inner(area);
        let viewport = usize::from(inner.height.max(1));

        let Some(pane) = &self.pane else {
            frame.render_widget(Paragraph::new("building preview...").block(block), area);
            return;
        };
        // Wrapped content occupies more display rows than logical lines;
        // clamp against the wrapped estimate or the tail becomes unreachable.
        let total = if pane.windowed {
            pane.lines.len()
        } else {
            wrapped_rows(&pane.lines, inner.width)
        };
        let max_scroll = u16::try_from(total.saturating_sub(viewport)).unwrap_or(u16::MAX);
        if self.scroll > max_scroll {
            self.scroll = max_scroll;
        }
        let position = Line::from(Span::styled(
            format!(" {}/{} ", self.scroll.saturating_add(1), total.max(1)),
            Style::default().fg(Color::Gray).add_modifier(Modifier::DIM),
        ));

        if pane.windowed {
            let window: Vec<Line<'static>> = pane
                .lines
                .iter()
                .skip(usize::from(self.scroll))
                .take(viewport)
                .cloned()
                .collect();
            frame.render_widget(
                Paragraph::new(Text::from(window)).block(block.title_top(position)),
                area,
            );
        } else {
            frame.render_widget(
                Paragraph::new(Text::from(pane.lines.clone()))
                    .wrap(Wrap { trim: false })
                    .scroll((self.scroll, 0))
                    .block(block.title_top(position)),
                area,
            );
        }
    }
}
