use std::{
    io::Write,
    path::Path,
    process::{Command, Stdio},
    sync::OnceLock,
};

use ansi_to_tui::IntoText as _;
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
};
use syntect::{
    easy::HighlightLines,
    highlighting::{Color as SyntectColor, FontStyle, Style as SyntectStyle, ThemeSet},
    parsing::SyntaxSet,
};

pub fn render_markdown_with_glow(body: &str, width: u16) -> Option<Vec<Line<'static>>> {
    if body.is_empty() || width == 0 {
        return None;
    }

    let mut child = Command::new("glow")
        .args(["-s", "dark", "-w", &width.to_string()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;

    let mut stdin = child.stdin.take()?;
    stdin.write_all(body.as_bytes()).ok()?;
    drop(stdin);

    let output = child.wait_with_output().ok()?;
    if !output.status.success() {
        return None;
    }

    let text = output.stdout.into_text().ok()?;
    Some(text_to_static_lines(text))
}

pub fn highlight_code(path: &str, source: &str) -> Vec<Line<'static>> {
    if source.is_empty() {
        return vec![Line::from(vec![
            Span::styled("  ", Style::default().fg(Color::DarkGray)),
            Span::styled("   1 ", Style::default().fg(Color::DarkGray)),
            Span::raw("no raw source"),
        ])];
    }

    let (syntax_set, theme_set) = syntax_assets();
    let syntax = syntax_set
        .find_syntax_by_path(path)
        .or_else(|| extension(path).and_then(|ext| syntax_set.find_syntax_by_extension(ext)))
        .unwrap_or_else(|| syntax_set.find_syntax_plain_text());
    let theme = theme_set
        .themes
        .get("base16-ocean.dark")
        .or_else(|| theme_set.themes.values().next());

    let Some(theme) = theme else {
        return numbered_plain_lines(source);
    };

    let mut highlighter = HighlightLines::new(syntax, theme);
    source
        .lines()
        .enumerate()
        .map(|(index, line)| {
            let mut spans = vec![
                Span::styled("  ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{:>4} ", index + 1),
                    Style::default().fg(Color::DarkGray),
                ),
            ];
            match highlighter.highlight_line(line, syntax_set) {
                Ok(ranges) => {
                    spans.extend(
                        ranges
                            .into_iter()
                            .filter(|(_, text)| !text.is_empty())
                            .map(|(style, text)| Span::styled(text.to_string(), tui_style(style))),
                    );
                }
                Err(_) => spans.push(Span::raw(line.to_string())),
            }
            Line::from(spans)
        })
        .collect()
}

fn numbered_plain_lines(source: &str) -> Vec<Line<'static>> {
    source
        .lines()
        .enumerate()
        .map(|(index, line)| {
            Line::from(vec![
                Span::styled("  ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{:>4} ", index + 1),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::raw(line.to_string()),
            ])
        })
        .collect()
}

fn syntax_assets() -> &'static (SyntaxSet, ThemeSet) {
    static ASSETS: OnceLock<(SyntaxSet, ThemeSet)> = OnceLock::new();
    ASSETS.get_or_init(|| {
        (
            SyntaxSet::load_defaults_newlines(),
            ThemeSet::load_defaults(),
        )
    })
}

fn extension(path: &str) -> Option<&str> {
    Path::new(path).extension().and_then(|ext| ext.to_str())
}

fn tui_style(style: SyntectStyle) -> Style {
    let mut modifier = Modifier::empty();
    if style.font_style.contains(FontStyle::BOLD) {
        modifier |= Modifier::BOLD;
    }
    if style.font_style.contains(FontStyle::ITALIC) {
        modifier |= Modifier::ITALIC;
    }
    if style.font_style.contains(FontStyle::UNDERLINE) {
        modifier |= Modifier::UNDERLINED;
    }

    Style::default()
        .fg(tui_color(style.foreground))
        .add_modifier(modifier)
}

fn tui_color(color: SyntectColor) -> Color {
    Color::Rgb(color.r, color.g, color.b)
}

fn text_to_static_lines(text: Text<'_>) -> Vec<Line<'static>> {
    text.lines
        .into_iter()
        .map(|line| {
            Line::from(
                line.spans
                    .into_iter()
                    .map(|span| Span::styled(span.content.into_owned(), span.style))
                    .collect::<Vec<_>>(),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line_text(line: &Line<'_>) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>()
    }

    #[test]
    fn highlights_rust_without_losing_text() {
        let lines = highlight_code("src/main.rs", "fn main() {\n    let value = 1;\n}");
        let text = lines.iter().map(line_text).collect::<Vec<_>>().join("\n");

        assert!(text.contains("fn main()"));
        assert!(
            lines
                .iter()
                .flat_map(|line| line.spans.iter())
                .any(|span| span.style != Style::default())
        );
    }

    #[test]
    fn glow_render_is_optional() {
        if Command::new("glow").arg("--version").output().is_err() {
            return;
        }

        let lines = render_markdown_with_glow("# Title\n\nbody", 40).expect("glow output");
        let text = lines.iter().map(line_text).collect::<Vec<_>>().join("\n");
        assert!(text.contains("Title"));
    }
}
