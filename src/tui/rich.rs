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

/// Markdown preview: prose renders through glow, fenced code blocks through
/// syntect. Glamour leaves fences without a language tag uncolored, so code
/// gets the same highlighter as the Code tab instead.
pub fn render_markdown_with_glow(body: &str, width: u16) -> Option<Vec<Line<'static>>> {
    if body.is_empty() || width == 0 {
        return None;
    }

    let segments = split_fences(body);
    // Reference-link definitions may live in a different segment than their
    // uses; hand every prose segment the full definition list so links keep
    // resolving. Glamour consumes definitions without rendering them.
    let definitions = if segments.len() > 1 {
        reference_definitions(body)
    } else {
        String::new()
    };
    let mut lines: Vec<Line<'static>> = Vec::new();
    for segment in segments {
        match segment {
            Segment::Prose(text) => {
                if text.trim().is_empty() {
                    continue;
                }
                let input = if definitions.is_empty() {
                    text
                } else {
                    format!("{text}\n{definitions}")
                };
                lines.extend(glow_lines(&input, width)?);
            }
            Segment::Code { language, source } => {
                if lines.last().is_some_and(|line| line.width() > 0) {
                    lines.push(Line::default());
                }
                lines.extend(highlight_fence(&language, &source));
                lines.push(Line::default());
            }
        }
    }
    Some(lines)
}

fn glow_lines(text: &str, width: u16) -> Option<Vec<Line<'static>>> {
    let style = glow_style_path()?;
    let mut child = Command::new("glow")
        .args(["-s", &style, "-w", &width.to_string()])
        // glow sees a pipe, not a terminal, and silently drops all color
        // without these.
        .env("CLICOLOR_FORCE", "1")
        .env("COLORTERM", "truecolor")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;

    // Feed stdin from a separate thread: writing a body larger than the OS
    // pipe buffer while glow's stdout pipe fills would deadlock both sides.
    let mut stdin = child.stdin.take()?;
    let body = text.as_bytes().to_vec();
    let writer = std::thread::spawn(move || {
        let _ = stdin.write_all(&body);
    });
    let output = child.wait_with_output().ok();
    let _ = writer.join();
    let output = output?;
    if !output.status.success() {
        return None;
    }

    let text = output.stdout.into_text().ok()?;
    Some(text_to_static_lines(text))
}

enum Segment {
    Prose(String),
    Code { language: String, source: String },
}

/// Collects `[label]: target` reference-link definition lines from the body.
fn reference_definitions(body: &str) -> String {
    body.lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            trimmed.starts_with('[') && trimmed.contains("]:")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Splits a markdown body at backtick fences. The fence lines are dropped;
/// the opening fence's info string becomes the code language. Per `CommonMark`,
/// a line indented four or more columns is a literal, never a fence; a block
/// closes only on a fence at least as long as the one that opened it, so a
/// four-backtick block can contain triple-backtick examples. An unclosed
/// fence keeps the rest of the body as `code`.
fn split_fences(body: &str) -> Vec<Segment> {
    let mut segments = Vec::new();
    let mut current = String::new();
    let mut open_fence: Option<(String, usize)> = None;
    for line in body.lines() {
        let trimmed = line.trim_start();
        let indent = line.chars().count() - trimmed.chars().count();
        let backticks = trimmed
            .chars()
            .take_while(|&character| character == '`')
            .count();
        let is_fence_line = backticks >= 3 && indent < 4;
        match (&open_fence, is_fence_line) {
            (Some((_, opened_with)), true)
                if backticks >= *opened_with && trimmed[backticks..].trim().is_empty() =>
            {
                let (language, _) = open_fence.take().expect("fence is open");
                segments.push(Segment::Code {
                    language,
                    source: std::mem::take(&mut current),
                });
            }
            (None, true) => {
                if !current.is_empty() {
                    segments.push(Segment::Prose(std::mem::take(&mut current)));
                }
                open_fence = Some((trimmed[backticks..].trim().to_string(), backticks));
            }
            _ => {
                current.push_str(line);
                current.push('\n');
            }
        }
    }
    if !current.is_empty() {
        segments.push(match open_fence {
            Some((language, _)) => Segment::Code {
                language,
                source: current,
            },
            None => Segment::Prose(current),
        });
    }
    segments
}

/// Highlights one fenced block with the Code tab's engine, indented by the
/// two columns glamour uses for code-block margins.
fn highlight_fence(language: &str, source: &str) -> Vec<Line<'static>> {
    let (syntax_set, theme_set) = syntax_assets();
    let syntax = syntax_set
        .find_syntax_by_token(language)
        .unwrap_or_else(|| syntax_set.find_syntax_plain_text());
    let Some(theme) = theme_set
        .themes
        .get("base16-ocean.dark")
        .or_else(|| theme_set.themes.values().next())
    else {
        return source
            .lines()
            .map(|line| Line::from(format!("  {line}")))
            .collect();
    };
    let mut highlighter = HighlightLines::new(syntax, theme);
    source
        .lines()
        .map(|line| {
            let mut spans = vec![Span::raw("  ")];
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

/// Glamour's built-in styles indent the whole document by a margin, which
/// floats the body off the pane's left border. Ship a dark style with the
/// document margin zeroed and hand it to glow as a style file.
fn glow_style_path() -> Option<String> {
    static STYLE_PATH: OnceLock<Option<String>> = OnceLock::new();
    STYLE_PATH
        .get_or_init(|| {
            let user = std::env::var("USER").unwrap_or_else(|_| "shared".to_string());
            let path = std::env::temp_dir().join(format!("forge-glow-style-{user}.json"));
            let staging = std::env::temp_dir().join(format!(
                "forge-glow-style-{user}-{}.json.tmp",
                std::process::id()
            ));
            // Write-then-rename keeps concurrent forge processes from ever
            // observing a truncated style file.
            std::fs::write(&staging, include_str!("glow_style.json")).ok()?;
            std::fs::rename(&staging, &path).ok()?;
            Some(path.to_string_lossy().into_owned())
        })
        .clone()
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

    #[test]
    fn reference_links_resolve_across_fence_segments() {
        if Command::new("glow").arg("--version").output().is_err() {
            return;
        }

        let body = "See [the docs][D].\n\n```sh\necho hi\n```\n\n[D]: https://example.com\n";
        let lines = render_markdown_with_glow(body, 60).expect("glow output");
        let text = lines.iter().map(line_text).collect::<Vec<_>>().join("\n");
        assert!(text.contains("https://example.com"));
        assert!(!text.contains("[D]"));
    }

    #[test]
    fn split_fences_separates_code_from_prose() {
        let segments = split_fences("intro\n\n```sh\necho hello\n```\n\noutro\n");
        assert_eq!(segments.len(), 3);
        let Segment::Code { language, source } = &segments[1] else {
            panic!("second segment should be code");
        };
        assert_eq!(language, "sh");
        assert_eq!(source, "echo hello\n");
        let Segment::Prose(outro) = &segments[2] else {
            panic!("third segment should be prose");
        };
        assert!(outro.contains("outro"));
    }

    #[test]
    fn split_fences_ignores_indented_literal_fence() {
        let segments = split_fences("prose\n\n    ```not-a-fence\n\nmore prose\n");
        assert_eq!(segments.len(), 1);
        let Segment::Prose(text) = &segments[0] else {
            panic!("all prose");
        };
        assert!(text.contains("```not-a-fence"));
    }

    #[test]
    fn split_fences_keeps_triple_backticks_inside_longer_fence() {
        let segments = split_fences("````markdown\n```sh\necho hi\n```\n````\n");
        assert_eq!(segments.len(), 1);
        let Segment::Code { language, source } = &segments[0] else {
            panic!("one code segment");
        };
        assert_eq!(language, "markdown");
        assert!(source.contains("```sh"));
        assert!(source.contains("echo hi"));
    }

    #[test]
    fn fenced_code_in_preview_gets_highlight_colors() {
        if Command::new("glow").arg("--version").output().is_err() {
            return;
        }

        let lines = render_markdown_with_glow("text\n\n```rust\nfn main() {}\n```\n", 60)
            .expect("glow output");
        let code_line = lines
            .iter()
            .find(|line| line_text(line).contains("fn main()"))
            .expect("code line present");
        assert!(
            code_line
                .spans
                .iter()
                .any(|span| span.style != Style::default())
        );
    }
}
