//! Gutter-preserving line wrapping. Long lines are pre-expanded into visual
//! rows so continuation text aligns after the gutter (line numbers, key
//! columns) with a `↪` marker, instead of sliding under it — the tuicr wrap
//! model. Pre-expansion also makes scroll math exact: one line, one row.

use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
};
use unicode_width::UnicodeWidthChar;

pub(super) fn expand_gutter_wrapped(
    lines: Vec<Line<'static>>,
    gutter_width: usize,
    viewport_width: usize,
) -> Vec<Line<'static>> {
    let viewport_width = viewport_width.max(1);
    let content_width = viewport_width.saturating_sub(gutter_width);
    if content_width < 8 {
        return lines;
    }
    let mut expanded = Vec::new();
    for line in lines {
        if line.width() <= viewport_width {
            expanded.push(line);
            continue;
        }
        let (gutter, content) = split_at_width(line.spans, gutter_width);
        for (row_index, row) in wrap_spans(content, content_width).into_iter().enumerate() {
            let mut spans = if row_index == 0 {
                gutter.clone()
            } else {
                vec![Span::styled(
                    format!("{:>width$} ", "↪", width = gutter_width.saturating_sub(1)),
                    Style::default().fg(Color::DarkGray),
                )]
            };
            spans.extend(row);
            expanded.push(Line::from(spans));
        }
    }
    expanded
}

/// Splits spans at a display-column boundary, cutting inside a span when the
/// boundary lands mid-span.
fn split_at_width(
    spans: Vec<Span<'static>>,
    width: usize,
) -> (Vec<Span<'static>>, Vec<Span<'static>>) {
    let mut left = Vec::new();
    let mut right = Vec::new();
    let mut consumed = 0usize;
    for span in spans {
        if consumed >= width {
            right.push(span);
            continue;
        }
        let span_width = span.width();
        if consumed + span_width <= width {
            consumed += span_width;
            left.push(span);
            continue;
        }
        let available = width - consumed;
        let mut taken = 0usize;
        let mut boundary = 0usize;
        for character in span.content.chars() {
            let character_width = character.width().unwrap_or(0);
            if taken + character_width > available {
                break;
            }
            taken += character_width;
            boundary += character.len_utf8();
        }
        let text = span.content.into_owned();
        left.push(Span::styled(text[..boundary].to_string(), span.style));
        right.push(Span::styled(text[boundary..].to_string(), span.style));
        consumed = width;
    }
    (left, right)
}

/// Breaks styled spans into rows of at most `width` display columns,
/// preserving each fragment's style. Character wrap, exact by construction.
fn wrap_spans(spans: Vec<Span<'static>>, width: usize) -> Vec<Vec<Span<'static>>> {
    let mut rows = Vec::new();
    let mut current: Vec<Span<'static>> = Vec::new();
    let mut current_width = 0usize;
    for span in spans {
        let style = span.style;
        let mut rest = span.content.into_owned();
        while !rest.is_empty() {
            let available = width.saturating_sub(current_width);
            let mut taken_width = 0usize;
            let mut boundary = 0usize;
            for character in rest.chars() {
                let character_width = character.width().unwrap_or(0);
                if taken_width + character_width > available {
                    break;
                }
                taken_width += character_width;
                boundary += character.len_utf8();
            }
            if boundary == 0 {
                if current.is_empty() {
                    // A single glyph wider than the row: force it through.
                    let character = rest.chars().next().expect("rest is non-empty");
                    boundary = character.len_utf8();
                    current.push(Span::styled(rest[..boundary].to_string(), style));
                    rest = rest[boundary..].to_string();
                }
                rows.push(std::mem::take(&mut current));
                current_width = 0;
                continue;
            }
            current.push(Span::styled(rest[..boundary].to_string(), style));
            current_width += taken_width;
            rest = rest[boundary..].to_string();
        }
    }
    if !current.is_empty() {
        rows.push(current);
    }
    if rows.is_empty() {
        rows.push(Vec::new());
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row_text(line: &Line<'_>) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect()
    }

    #[test]
    fn short_lines_pass_through_unchanged() {
        let lines = vec![Line::from("  12 short line")];
        let expanded = expand_gutter_wrapped(lines.clone(), 5, 40);
        assert_eq!(expanded.len(), 1);
        assert_eq!(row_text(&expanded[0]), row_text(&lines[0]));
    }

    #[test]
    fn long_lines_continue_after_the_gutter_with_marker() {
        let gutter = "  12 ";
        let content = "a".repeat(60);
        let lines = vec![Line::from(format!("{gutter}{content}"))];
        let expanded = expand_gutter_wrapped(lines, 5, 30);
        assert_eq!(expanded.len(), 3);
        assert!(row_text(&expanded[0]).starts_with("  12 "));
        assert!(row_text(&expanded[1]).starts_with("   ↪ "));
        assert_eq!(row_text(&expanded[1]).chars().count(), 30);
        let rejoined: String = expanded
            .iter()
            .map(|line| row_text(line).chars().skip(5).collect::<String>())
            .collect();
        assert_eq!(rejoined, "a".repeat(60));
    }

    #[test]
    fn styles_survive_the_wrap_boundary() {
        let styled = Span::styled("b".repeat(40), Style::default().fg(Color::Green));
        let lines = vec![Line::from(vec![Span::raw("  1 "), styled])];
        let expanded = expand_gutter_wrapped(lines, 4, 24);
        assert!(expanded.len() > 1);
        for line in &expanded {
            assert!(
                line.spans
                    .iter()
                    .any(|span| span.style.fg == Some(Color::Green))
            );
        }
    }
}
