use std::{
    collections::BTreeMap,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc::{self, Receiver},
    thread,
};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Position, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
};

use commands::{
    services::{
        self, builders,
        files::{self, FileSections},
    },
    view::{
        Adr, ArtifactView, Companion, DashboardView, ModuleView, ProvenanceArtifact, StatusSummary,
        VcsState, WorktreeState,
    },
};

use crate::cli::{config, watchlist};

use super::components::{
    palette::{Palette, PaletteCommand},
    preview::{ArtifactPreview, wrapped_rows},
};
use super::rich;
use super::word_wrap::expand_gutter_wrapped;

const SECTION_COUNT: usize = 13;
const DETAIL_TAB_COUNT: usize = 7;
const LEFT_MIN_WIDTH: u16 = 14;
const LEFT_MAX_WIDTH: u16 = 20;
const MIDDLE_MIN_WIDTH: u16 = 24;
const MIDDLE_MAX_WIDTH: u16 = 40;
const MIN_DETAIL_WIDTH: u16 = 20;
/// Columns occupied by the code gutter: comment marker (2) plus a
/// right-aligned line number (4) plus one space.
const CODE_GUTTER: usize = 7;

pub const KEYBINDINGS: &[(&str, &[(&str, &str)])] = &[
    (
        "Navigation",
        &[
            ("h/j/k/l", "move, drill, and go back"),
            ("arrows", "move, drill, and go back"),
            ("Tab", "next column or detail tab"),
            ("BackTab", "previous column"),
            ("Enter", "drill or expand detail"),
            ("Esc", "back, close overlay, or quit"),
            ("g/G", "top or bottom"),
            ("PgUp/PgDn", "scroll detail"),
        ],
    ),
    (
        "Sections",
        &[
            ("1", "overview"),
            ("2", "skills"),
            ("3", "agents"),
            ("4", "rules"),
            ("5", "repositories"),
            ("6", "ADRs"),
            ("7", "provenance"),
            ("8", "variants"),
            ("9", "search"),
            ("t", "settings"),
            ("h", "hooks"),
            ("c", "config"),
            ("m", "schemas"),
        ],
    ),
    (
        "Actions",
        &[
            ("/", "search"),
            (":", "palette"),
            ("r", "refresh"),
            ("y", "copy install snippet or path"),
            ("Tab", "next detail tab"),
            ("p c d v f i n", "detail tabs"),
            ("m", "comment line (from any detail tab)"),
            ("Y", "copy tuicr comments"),
            ("o/O", "open gitui / jjui on repository"),
        ],
    ),
    ("Global", &[("?", "help"), ("F1", "help"), ("q", "quit")]),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnFocus {
    Sections,
    List,
    Detail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Overview = 0,
    Skills = 1,
    Agents = 2,
    Rules = 3,
    Repositories = 4,
    Adrs = 5,
    Provenance = 6,
    Variants = 7,
    Search = 8,
    Settings = 9,
    Hooks = 10,
    Config = 11,
    Schemas = 12,
}

impl Section {
    const ALL: [Self; SECTION_COUNT] = [
        Self::Overview,
        Self::Skills,
        Self::Agents,
        Self::Rules,
        Self::Repositories,
        Self::Adrs,
        Self::Provenance,
        Self::Variants,
        Self::Search,
        Self::Settings,
        Self::Hooks,
        Self::Config,
        Self::Schemas,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Skills => "Skills",
            Self::Agents => "Agents",
            Self::Rules => "Rules",
            Self::Repositories => "Repositories",
            Self::Adrs => "ADRs",
            Self::Provenance => "Provenance",
            Self::Variants => "Variants",
            Self::Search => "Search",
            Self::Settings => "Settings",
            Self::Hooks => "Hooks",
            Self::Config => "Config",
            Self::Schemas => "Schemas",
        }
    }

    fn from_index(index: usize) -> Self {
        Self::ALL[index.min(SECTION_COUNT - 1)]
    }

    fn from_name(name: &str) -> Option<Self> {
        match name {
            "overview" | "o" => Some(Self::Overview),
            "skills" | "skill" | "s" => Some(Self::Skills),
            "agents" | "agent" | "a" => Some(Self::Agents),
            "rules" | "rule" => Some(Self::Rules),
            "repos" | "repositories" | "repository" => Some(Self::Repositories),
            "adrs" | "adr" => Some(Self::Adrs),
            "provenance" | "integrity" => Some(Self::Provenance),
            "variants" | "variant" => Some(Self::Variants),
            "search" | "find" => Some(Self::Search),
            "settings" | "setting" => Some(Self::Settings),
            "hooks" | "hook" => Some(Self::Hooks),
            "config" | "configuration" => Some(Self::Config),
            "schemas" | "schema" | "manifests" | "manifest" => Some(Self::Schemas),
            _ => None,
        }
    }

    /// The key that reaches this section from the Sections column, shown as
    /// the row prefix so every advertised shortcut actually works.
    fn shortcut_label(self) -> &'static str {
        match self {
            Self::Overview => "1",
            Self::Skills => "2",
            Self::Agents => "3",
            Self::Rules => "4",
            Self::Repositories => "5",
            Self::Adrs => "6",
            Self::Provenance => "7",
            Self::Variants => "8",
            Self::Search => "9",
            Self::Settings => "t",
            Self::Hooks => "h",
            Self::Config => "c",
            Self::Schemas => "m",
        }
    }

    fn from_shortcut(character: char) -> Option<Self> {
        match character {
            '1' => Some(Self::Overview),
            '2' => Some(Self::Skills),
            '3' => Some(Self::Agents),
            '4' => Some(Self::Rules),
            '5' => Some(Self::Repositories),
            '6' => Some(Self::Adrs),
            '7' => Some(Self::Provenance),
            '8' => Some(Self::Variants),
            '9' => Some(Self::Search),
            't' | 'T' => Some(Self::Settings),
            'h' | 'H' => Some(Self::Hooks),
            'c' | 'C' => Some(Self::Config),
            'm' | 'M' => Some(Self::Schemas),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetailTab {
    Preview = 0,
    Code = 1,
    Diff = 2,
    Provenance = 3,
    Frontmatter = 4,
    History = 5,
    Companions = 6,
}

impl DetailTab {
    pub(super) const ALL: [Self; DETAIL_TAB_COUNT] = [
        Self::Preview,
        Self::Code,
        Self::Diff,
        Self::Provenance,
        Self::Frontmatter,
        Self::History,
        Self::Companions,
    ];

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Preview => "Preview",
            Self::Code => "Code",
            Self::Diff => "Diff",
            Self::Provenance => "Provenance",
            Self::Frontmatter => "Frontmatter",
            Self::History => "History",
            Self::Companions => "Companions",
        }
    }

    fn from_index(index: usize) -> Self {
        Self::ALL[index.min(DETAIL_TAB_COUNT - 1)]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScanState {
    Idle,
    Loading,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RunState {
    Running,
    Quit,
}

struct ScanResult {
    providers: Vec<(String, String)>,
    watched_locations: Vec<PathBuf>,
    view: DashboardView,
    file_sections: FileSections,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OverviewMode {
    Nested,
    Matrix,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HelpState {
    Closed,
    Open,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ListTarget {
    None,
    Overview,
    /// The Overview Nested/Matrix mode row: activating it toggles the mode.
    OverviewMode,
    /// A skill companion file, shown as a child row under its parent skill.
    Companion {
        module: String,
        parent: String,
        name: String,
    },
    Artifact {
        module: String,
        kind: String,
        name: String,
    },
    Module(String),
    Adr {
        repo: String,
        id: String,
    },
    ProvenanceArtifact {
        module: String,
        kind: String,
        name: String,
    },
    Variant {
        module: String,
        kind: String,
        name: String,
        qualifier: String,
    },
    SettingsFile {
        group: usize,
        index: usize,
    },
    Hook {
        group: usize,
        index: usize,
    },
    ConfigFile(usize),
    SchemaFile {
        group: usize,
        index: usize,
    },
}

#[derive(Debug, Clone)]
struct ListRow {
    label: String,
    detail: String,
    target: ListTarget,
    header: bool,
    status: &'static str,
}

impl ListRow {
    fn header(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            detail: String::new(),
            target: ListTarget::None,
            header: true,
            status: "source",
        }
    }

    fn item(
        label: impl Into<String>,
        detail: impl Into<String>,
        target: ListTarget,
        status: &'static str,
    ) -> Self {
        Self {
            label: label.into(),
            detail: detail.into(),
            target,
            header: false,
            status,
        }
    }

    fn is_selectable(&self) -> bool {
        !self.header && !matches!(self.target, ListTarget::None)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MillerColumnWidths {
    pub left: u16,
    pub middle: u16,
}

/// Screen rectangles captured during render so mouse events can be
/// hit-tested against what is actually on screen.
#[derive(Debug, Clone, Copy, Default)]
struct MouseRegions {
    sections: Rect,
    list: Rect,
    detail: Rect,
    tabs: Rect,
}

/// Rendered lines for the current detail view, rebuilt only when the target,
/// tab, or pane width changes — per-frame rebuilds are what made the detail
/// pane feel slow.
#[derive(Debug, Clone, PartialEq, Eq)]
struct DetailCache {
    key: String,
    width: u16,
    lines: Vec<Line<'static>>,
    /// Lines already wrapped at the pane width (glow output): render a
    /// scrolled window without Paragraph wrap, which would break tables.
    windowed: bool,
    /// Row offsets of diff hunk headers, for [ and ] navigation.
    hunks: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CodeCache {
    path: String,
    lines: Vec<Line<'static>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CommentKind {
    Issue,
    Note,
    Suggestion,
    Praise,
}

impl CommentKind {
    pub fn next(self) -> Self {
        match self {
            Self::Issue => Self::Note,
            Self::Note => Self::Suggestion,
            Self::Suggestion => Self::Praise,
            Self::Praise => Self::Issue,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Issue => "ISSUE",
            Self::Note => "NOTE",
            Self::Suggestion => "SUGGESTION",
            Self::Praise => "PRAISE",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LineComment {
    kind: CommentKind,
    text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CommentPrompt {
    module: String,
    path: String,
    line_number: usize,
    kind: CommentKind,
    text: String,
}

pub struct App {
    root: PathBuf,
    providers: Vec<(String, String)>,
    watched_locations: Vec<PathBuf>,
    view: DashboardView,
    file_sections: FileSections,
    scan_receiver: Option<Receiver<Result<ScanResult, String>>>,
    scan_state: ScanState,
    focused: ColumnFocus,
    section: Section,
    cached_rows: Vec<ListRow>,
    column_widths: MillerColumnWidths,
    rows_dirty: bool,
    #[cfg(test)]
    row_build_count: usize,
    preview_cache: Option<DetailCache>,
    code_cache: Option<CodeCache>,
    comments: BTreeMap<(String, String, usize), LineComment>,
    comment_prompt: Option<CommentPrompt>,
    #[cfg(test)]
    preview_cache_build_count: usize,
    #[cfg(test)]
    code_cache_build_count: usize,
    list_selected: [usize; SECTION_COUNT],
    detail_tab: DetailTab,
    detail_scroll: u16,
    overview_mode: OverviewMode,
    search: builders::SearchFilters,
    run_state: RunState,
    pub palette_error: Option<String>,
    toast: Option<String>,
    preview: Option<ArtifactPreview>,
    help_state: HelpState,
    palette: Palette,
    mouse_regions: MouseRegions,
    /// External TUI (gitui/jjui) queued to run in a repo; the event loop
    /// suspends the terminal, runs it, and resumes.
    pending_external: Option<(String, PathBuf)>,
    /// First visible row of the list column (viewport scroll offset).
    list_offset: usize,
    /// Selection seen at the last render, to detect selection movement.
    list_last_selected: usize,
    /// Second-press confirmation state for quitting with unsaved comments.
    quit_armed: bool,
    /// Line cursor for the Code tab, decoupled from the viewport: keys move
    /// it (viewport follows), the wheel scrolls without touching it.
    code_cursor: usize,
    /// Detail body height at the last render, for cursor-follow and paging.
    detail_viewport: usize,
    /// Synthesized artifact for the selected ADR or companion, keyed by a
    /// stable identity so tab switches do not re-read files or re-run git.
    synthesized: Option<(String, ArtifactView)>,
    /// Whether keystrokes in the Search section edit the query (explicit
    /// input mode) or navigate the result list.
    search_typing: bool,
}

impl App {
    pub fn load(root: PathBuf) -> Self {
        let mut app = Self::from_view(root, Vec::new(), Vec::new(), empty_dashboard_view());
        app.start_scan();
        app
    }

    #[must_use]
    pub fn from_view(
        root: PathBuf,
        providers: Vec<(String, String)>,
        watched_locations: Vec<PathBuf>,
        view: DashboardView,
    ) -> Self {
        Self::from_view_with_files(
            root,
            providers,
            watched_locations,
            view,
            FileSections::default(),
        )
    }

    #[must_use]
    pub fn from_view_with_files(
        root: PathBuf,
        providers: Vec<(String, String)>,
        watched_locations: Vec<PathBuf>,
        view: DashboardView,
        file_sections: FileSections,
    ) -> Self {
        Self {
            root,
            providers,
            watched_locations,
            view,
            file_sections,
            scan_receiver: None,
            scan_state: ScanState::Idle,
            focused: ColumnFocus::Sections,
            section: Section::Overview,
            cached_rows: Vec::new(),
            column_widths: default_column_widths(),
            rows_dirty: true,
            #[cfg(test)]
            row_build_count: 0,
            preview_cache: None,
            code_cache: None,
            comments: BTreeMap::new(),
            comment_prompt: None,
            #[cfg(test)]
            preview_cache_build_count: 0,
            #[cfg(test)]
            code_cache_build_count: 0,
            list_selected: [0; SECTION_COUNT],
            detail_tab: DetailTab::Preview,
            detail_scroll: 0,
            overview_mode: OverviewMode::Nested,
            search: builders::SearchFilters::empty(),
            run_state: RunState::Running,
            palette_error: None,
            toast: None,
            preview: None,
            help_state: HelpState::Closed,
            palette: Palette::new(),
            mouse_regions: MouseRegions::default(),
            pending_external: None,
            list_offset: 0,
            list_last_selected: 0,
            quit_armed: false,
            code_cursor: 0,
            detail_viewport: 1,
            synthesized: None,
            search_typing: false,
        }
    }

    pub fn refresh(&mut self) {
        if self.scan_state == ScanState::Loading {
            self.toast = Some("scan already running".to_string());
            return;
        }
        self.start_scan();
    }

    /// Restarts the scan even when one is in flight, superseding its result —
    /// used after an external tool may have changed the repos.
    pub fn force_refresh(&mut self) {
        self.start_scan();
    }

    /// Whether a background scan is still in flight (used by snapshot mode to
    /// block until real data is available before rendering a frame).
    #[must_use]
    pub fn scan_pending(&self) -> bool {
        self.scan_receiver.is_some()
    }

    pub fn poll_scan(&mut self) {
        let Some(receiver) = &self.scan_receiver else {
            return;
        };
        match receiver.try_recv() {
            Ok(Ok(scan_result)) => {
                self.providers = scan_result.providers;
                self.watched_locations = scan_result.watched_locations;
                self.view = scan_result.view;
                self.file_sections = scan_result.file_sections;
                self.scan_state = ScanState::Idle;
                self.scan_receiver = None;
                self.toast = Some("scan complete".to_string());
                let previous_target = self.selected_target();
                self.invalidate_rows();
                self.invalidate_detail_caches();
                self.refresh_open_preview();
                self.restore_selection(previous_target);
                self.clamp_list_selection();
            }
            Ok(Err(error)) => {
                self.scan_state = ScanState::Idle;
                self.scan_receiver = None;
                self.palette_error = Some(error);
            }
            Err(mpsc::TryRecvError::Empty) => {}
            Err(mpsc::TryRecvError::Disconnected) => {
                self.scan_state = ScanState::Idle;
                self.scan_receiver = None;
                self.palette_error = Some("scan worker disconnected".to_string());
            }
        }
    }

    fn start_scan(&mut self) {
        let root = self.root.clone();
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let providers = load_provider_targets(&root);
            let watched_locations = watchlist::watched_locations();
            let settings_filenames = config::load_settings_filenames(&root);
            let result = services::build_view(&root, &providers, &watched_locations)
                .map(|view| {
                    let local_repos = services::discover_local_repos(&root, &watched_locations);
                    let allowed_sources = services::active_repo_names(&view.modules, &root);
                    let file_sections = files::collect_file_sections(
                        &root,
                        &providers,
                        &settings_filenames,
                        &local_repos,
                        &allowed_sources,
                    );
                    ScanResult {
                        providers,
                        watched_locations,
                        view,
                        file_sections,
                    }
                })
                .map_err(|error| format!("{error}"));
            let _ = sender.send(result);
        });
        self.scan_receiver = Some(receiver);
        self.scan_state = ScanState::Loading;
        self.palette_error = None;
        self.toast = None;
    }

    pub fn render(&mut self, frame: &mut Frame<'_>) {
        if self.preview.is_some() {
            let area = frame.area();
            let inner_width = area.width.saturating_sub(2).max(1);
            let tab = self.detail_tab;
            let needs_rebuild = self
                .preview
                .as_ref()
                .is_some_and(|preview| preview.needs_rebuild(tab, inner_width));
            if needs_rebuild {
                let artifact = self
                    .preview
                    .as_ref()
                    .map(|preview| preview.artifact().clone())
                    .expect("preview is open");
                let (lines, windowed) = {
                    let module = self
                        .view
                        .modules
                        .iter()
                        .find(|module| module.name == artifact.module);
                    self.build_detail_lines(module, &artifact, tab, inner_width)
                };
                if let Some(preview) = self.preview.as_mut() {
                    preview.set_lines(tab, inner_width, lines, windowed);
                }
            }
            if let Some(preview) = self.preview.as_mut() {
                preview.render(frame, area);
            }
            return;
        }

        self.ensure_rows();
        self.clamp_list_selection();
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Min(1),
                Constraint::Length(1),
            ])
            .split(frame.area());
        self.render_status(frame, layout[0]);
        let fitted_widths = fit_miller_widths(layout[1].width, self.column_widths);
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(fitted_widths.left),
                Constraint::Length(fitted_widths.middle),
                Constraint::Min(0),
            ])
            .split(layout[1]);
        self.mouse_regions.sections = columns[0];
        self.mouse_regions.list = columns[1];
        self.mouse_regions.detail = columns[2];
        self.mouse_regions.tabs = Rect::default();
        self.render_sections(frame, columns[0]);
        self.render_list(frame, columns[1]);
        self.render_detail(frame, columns[2]);
        self.render_footer(frame, layout[2]);

        if self.help_state == HelpState::Open {
            render_help(frame, frame.area());
        }
    }

    fn render_status(&self, frame: &mut Frame<'_>, area: Rect) {
        let scan = if self.scan_state == ScanState::Loading {
            "Scanning modules..."
        } else {
            "ready"
        };
        let summary = &self.view.summary;
        let comments = if self.comments.is_empty() {
            String::new()
        } else {
            format!(" | ✎ {} comments (Y copies)", self.comments.len())
        };
        let text = format!(
            " forge tui | {scan} | ok {} stale {} modified {} new {} | {} modules{comments}",
            summary.unchanged,
            summary.stale,
            summary.modified,
            summary.new,
            self.view.modules.len()
        );
        frame.render_widget(
            Paragraph::new(text).style(Style::default().fg(Color::Gray)),
            area,
        );
    }

    fn render_footer(&self, frame: &mut Frame<'_>, area: Rect) {
        let text = if let Some(prompt) = &self.comment_prompt {
            format!(
                " comment [{}] {}:{} > {}",
                prompt.kind.label(),
                prompt.path,
                prompt.line_number,
                prompt.text
            )
        } else if self.palette.is_open() || self.palette_error.is_some() {
            self.palette.display_text(self.palette_error.as_deref())
        } else if let Some(toast) = &self.toast {
            format!(" {toast}")
        } else if let Some((current, total)) = self.hunk_position() {
            format!("hunk {current}/{total}  ·  ] next hunk  ·  [ previous hunk  ·  j/k scroll")
        } else {
            hint_row(self.focused)
        };
        frame.render_widget(
            Paragraph::new(text).style(Style::default().fg(Color::DarkGray)),
            area,
        );
    }

    fn render_sections(&self, frame: &mut Frame<'_>, area: Rect) {
        let block = column_block(" Sections ", self.focused == ColumnFocus::Sections);
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let items: Vec<ListItem<'_>> = Section::ALL
            .iter()
            .enumerate()
            .map(|(index, section)| {
                let _ = index;
                let prefix = format!("{} ", section.shortcut_label());
                let style = if *section == self.section {
                    selected_style(self.focused == ColumnFocus::Sections)
                } else {
                    Style::default()
                };
                ListItem::new(Line::from(vec![
                    Span::styled(prefix, Style::default().fg(Color::DarkGray)),
                    Span::styled(section.label(), style),
                ]))
            })
            .collect();
        frame.render_widget(List::new(items), inner);
    }

    fn render_list(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let title = format!(" {} ", self.section.label());
        let block = column_block(&title, self.focused == ColumnFocus::List);
        let inner = block.inner(area);
        frame.render_widget(block, area);

        if self.scan_state == ScanState::Loading && self.cached_rows.is_empty() {
            frame.render_widget(
                Paragraph::new("Scanning modules...").style(Style::default().fg(Color::Gray)),
                inner,
            );
            return;
        }
        let viewport = usize::from(inner.height.max(1));
        let selected = self.selected_list_index(&self.cached_rows);
        // The viewport follows selection changes (keyboard/click), while wheel
        // scrolling moves the offset alone — passive gestures never drag the
        // selection, and moving the selection always brings it back on screen.
        if selected != self.list_last_selected {
            if selected < self.list_offset {
                self.list_offset = selected;
            } else if selected + 1 > self.list_offset + viewport {
                self.list_offset = selected + 1 - viewport;
            }
            self.list_last_selected = selected;
        }
        self.list_offset = self
            .list_offset
            .min(self.cached_rows.len().saturating_sub(viewport));
        let offset = self.list_offset;
        let rows = &self.cached_rows;
        let items: Vec<ListItem<'_>> = if rows.is_empty() {
            vec![ListItem::new("no rows")]
        } else {
            rows.iter()
                .enumerate()
                .skip(offset)
                .take(viewport)
                .map(|(index, row)| {
                    if row.header {
                        return ListItem::new(Line::from(Span::styled(
                            row.label.clone(),
                            Style::default()
                                .fg(Color::Magenta)
                                .add_modifier(Modifier::BOLD),
                        )));
                    }
                    let base = if index == selected {
                        selected_style(self.focused == ColumnFocus::List)
                    } else {
                        Style::default()
                    };
                    let mut spans = vec![
                        Span::styled(status_dot(row.status), status_style(row.status)),
                        Span::raw(" "),
                        Span::styled(row.label.clone(), base),
                    ];
                    // Detail text only on the selected row: unselected rows
                    // stay calm and nothing truncates while browsing.
                    if index == selected && !row.detail.is_empty() {
                        spans.push(Span::styled(
                            format!("  {}", row.detail),
                            base.fg(Color::DarkGray),
                        ));
                    }
                    ListItem::new(Line::from(spans))
                })
                .collect()
        };
        frame.render_widget(List::new(items), inner);
    }

    fn render_detail(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let position = match self.detail_tab {
            DetailTab::Code => self
                .code_cache
                .as_ref()
                .map(|cache| (self.code_cursor + 1, cache.lines.len())),
            _ => self
                .preview_cache
                .as_ref()
                .map(|cache| (usize::from(self.detail_scroll) + 1, cache.lines.len())),
        };
        let title = match position {
            Some((current, total)) if total > 0 => {
                format!(" Detail · {}/{total} ", current.min(total))
            }
            _ => " Detail ".to_string(),
        };
        let block = column_block(&title, self.focused == ColumnFocus::Detail);
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let target = self.selected_target();
        match target {
            Some(
                ListTarget::Artifact { module, kind, name }
                | ListTarget::ProvenanceArtifact { module, kind, name },
            ) => {
                if let Some((module_index, artifact_index)) =
                    self.find_artifact_indices(&module, &kind, &name)
                {
                    self.render_artifact_detail(frame, inner, module_index, artifact_index);
                } else {
                    frame.render_widget(Paragraph::new("artifact not found"), inner);
                }
            }
            Some(ListTarget::Adr { repo, id }) => {
                if let Some(adr) = self.find_adr(&repo, &id).cloned() {
                    let identity = format!("adr:{}", adr.local_path);
                    let module_name = adr.repo.clone();
                    self.render_synthesized_detail(frame, inner, &identity, &module_name, |app| {
                        app.build_adr_artifact_view(&adr)
                    });
                } else {
                    frame.render_widget(Paragraph::new("ADR not found"), inner);
                }
            }
            Some(ListTarget::Companion {
                module,
                parent,
                name,
            }) => {
                self.render_companion_detail(frame, inner, &module, &parent, &name);
            }
            Some(ListTarget::Module(name)) => {
                if let Some(module) = self.view.modules.iter().find(|module| module.name == name) {
                    render_module_detail(frame, inner, module, self.detail_scroll);
                } else {
                    frame.render_widget(Paragraph::new("repository not found"), inner);
                }
            }
            Some(ListTarget::Variant {
                module,
                kind,
                name,
                qualifier,
            }) => {
                self.render_variant_detail(frame, inner, &module, &kind, &name, &qualifier);
            }
            Some(ListTarget::SettingsFile { group, index }) => {
                if let Some(file) = self.settings_file(group, index) {
                    render_file_body(frame, inner, &file.content, self.detail_scroll);
                } else {
                    frame.render_widget(Paragraph::new("settings file not found"), inner);
                }
            }
            Some(ListTarget::Hook { group, index }) => {
                if let Some(hook) = self.hook_entry(group, index) {
                    render_hook_detail(frame, inner, hook, self.detail_scroll);
                } else {
                    frame.render_widget(Paragraph::new("hook not found"), inner);
                }
            }
            Some(ListTarget::ConfigFile(index)) => {
                if let Some(file) = self.file_sections.config.get(index) {
                    render_file_body(frame, inner, &file.content, self.detail_scroll);
                } else {
                    frame.render_widget(Paragraph::new("config file not found"), inner);
                }
            }
            Some(ListTarget::SchemaFile { group, index }) => {
                if let Some(file) = self.schema_file(group, index) {
                    render_file_body(frame, inner, &file.content, self.detail_scroll);
                } else {
                    frame.render_widget(Paragraph::new("schema file not found"), inner);
                }
            }
            _ => self.render_overview_detail(frame, inner),
        }
    }

    fn render_artifact_detail(
        &mut self,
        frame: &mut Frame<'_>,
        area: Rect,
        module_index: usize,
        artifact_index: usize,
    ) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(2), Constraint::Min(1)])
            .split(area);
        self.mouse_regions.tabs = chunks[0];
        self.render_tabs(frame, chunks[0]);
        self.prepare_artifact_detail_cache(module_index, artifact_index, chunks[1].width);
        if self.detail_tab == DetailTab::Code {
            let viewport = usize::from(chunks[1].height.max(1));
            self.detail_viewport = viewport;
            // The cursor is the top visible line, so every line must be able
            // to reach the top — clamp to the last line, not the last page.
            let max_scroll = self
                .code_cache
                .as_ref()
                .map_or(0, |cache| cache.lines.len())
                .saturating_sub(1);
            self.detail_scroll = self
                .detail_scroll
                .min(u16::try_from(max_scroll).unwrap_or(u16::MAX));
            let artifact = &self.view.modules[module_index].artifacts[artifact_index];
            let lines = self.code_window(artifact, viewport);
            // Pre-expand long lines so continuation rows align after the
            // line-number gutter with a ↪ marker instead of sliding under it.
            let mut rows = expand_gutter_wrapped(lines, CODE_GUTTER, usize::from(chunks[1].width));
            rows.truncate(viewport);
            frame.render_widget(Paragraph::new(Text::from(rows)), chunks[1]);
        } else {
            let expected_key = {
                let module = &self.view.modules[module_index];
                let artifact = &module.artifacts[artifact_index];
                detail_cache_key(self.detail_tab, &module.name, &artifact.relative_path)
            };
            // A deferred rebuild (input still queued) leaves the previous
            // artifact's lines in the cache; render a placeholder rather than
            // content that belongs to another selection.
            if self
                .preview_cache
                .as_ref()
                .is_some_and(|cache| cache.key != expected_key)
            {
                frame.render_widget(
                    Paragraph::new("rendering…").style(Style::default().fg(Color::DarkGray)),
                    chunks[1],
                );
            } else {
                self.render_cached_detail(frame, chunks[1]);
            }
        }
    }

    /// Full ADR document rendered through the markdown pipeline, replacing the
    /// one-paragraph summary that used to cut the body off.
    /// Renders a synthesized artifact (ADR, companion) through the same
    /// tabbed detail pipeline as scanned artifacts: one artifact view
    /// everywhere.
    fn render_synthesized_detail(
        &mut self,
        frame: &mut Frame<'_>,
        area: Rect,
        identity: &str,
        module_name: &str,
        build: impl FnOnce(&Self) -> ArtifactView,
    ) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(2), Constraint::Min(1)])
            .split(area);
        self.mouse_regions.tabs = chunks[0];
        self.render_tabs(frame, chunks[0]);
        let cache_width = chunks[1].width.max(1);
        let key = detail_cache_key(self.detail_tab, module_name, identity);
        let needs_build = self
            .preview_cache
            .as_ref()
            .is_none_or(|cache| cache.key != key || cache.width != cache_width);
        if needs_build {
            if self.preview_cache.is_some() && input_pending() {
                frame.render_widget(
                    Paragraph::new("rendering…").style(Style::default().fg(Color::DarkGray)),
                    chunks[1],
                );
                return;
            }
            if self
                .preview_cache
                .as_ref()
                .is_some_and(|cache| cache.key != key)
            {
                self.detail_scroll = 0;
            }
            if self
                .synthesized
                .as_ref()
                .is_none_or(|(cached, _)| cached != identity)
            {
                let artifact = build(self);
                self.synthesized = Some((identity.to_string(), artifact));
            }
            let (lines, windowed) = {
                let (_, artifact) = self.synthesized.as_ref().expect("synthesized just set");
                let module = self
                    .view
                    .modules
                    .iter()
                    .find(|module| module.name == module_name);
                self.build_detail_lines(module, artifact, self.detail_tab, cache_width)
            };
            let hunks = hunk_offsets(&lines);
            self.preview_cache = Some(DetailCache {
                key,
                width: cache_width,
                lines,
                windowed,
                hunks,
            });
        }
        self.render_cached_detail(frame, chunks[1]);
    }

    fn render_companion_detail(
        &mut self,
        frame: &mut Frame<'_>,
        area: Rect,
        module: &str,
        parent: &str,
        name: &str,
    ) {
        let found = self
            .view
            .modules
            .iter()
            .find(|candidate| candidate.name == module)
            .and_then(|candidate| {
                candidate
                    .artifacts
                    .iter()
                    .find(|artifact| artifact.name == parent)
            })
            .and_then(|artifact| {
                artifact
                    .companions
                    .iter()
                    .find(|companion| companion.name == name)
            })
            .cloned();
        if let Some(companion) = found {
            let identity = format!("companion:{module}:{parent}:{name}");
            self.render_synthesized_detail(frame, area, &identity, module, |app| {
                app.build_companion_artifact_view(module, parent, &companion)
            });
        } else {
            frame.render_widget(Paragraph::new("companion not found"), area);
        }
    }

    /// One artifact view for an ADR: full raw source, stripped body,
    /// frontmatter, per-file git history, and the module's VCS state.
    fn build_adr_artifact_view(&self, adr: &Adr) -> ArtifactView {
        let raw = std::fs::read_to_string(&adr.local_path)
            .unwrap_or_else(|error| format!("could not read {}: {error}", adr.local_path));
        let body = services::strip_frontmatter(&raw);
        let module = self
            .view
            .modules
            .iter()
            .find(|module| module.name == adr.repo);
        let git_log = module
            .and_then(|module| module.local_path.as_ref())
            .map(|repo| services::git_log_in_repo(repo, &adr.relative_path))
            .unwrap_or_default();
        ArtifactView {
            name: format!("{} {}", adr.id, adr.title),
            kind: "adr".to_string(),
            module: adr.repo.clone(),
            relative_path: adr.relative_path.clone(),
            source_path: adr.relative_path.clone(),
            description: format!("{} · {}", adr.state, adr.status),
            metadata: services::parse_frontmatter(&raw),
            content_body: body,
            raw_source: raw,
            git_log,
            vcs: module.and_then(|module| module.vcs.clone()),
            ..ArtifactView::default()
        }
    }

    /// One artifact view for a skill companion file.
    fn build_companion_artifact_view(
        &self,
        module_name: &str,
        parent: &str,
        companion: &Companion,
    ) -> ArtifactView {
        let module = self
            .view
            .modules
            .iter()
            .find(|module| module.name == module_name);
        let git_log = module
            .and_then(|module| module.local_path.as_ref())
            .map(|repo| services::git_log_in_repo(repo, &companion.relative_path))
            .unwrap_or_default();
        ArtifactView {
            name: format!("{parent}/{}", companion.name),
            kind: "companion".to_string(),
            module: module_name.to_string(),
            relative_path: companion.relative_path.clone(),
            source_path: companion.relative_path.clone(),
            description: companion.description.clone(),
            metadata: services::parse_frontmatter(&companion.raw_source),
            content_body: companion.content_body.clone(),
            raw_source: companion.raw_source.clone(),
            git_log,
            vcs: module.and_then(|module| module.vcs.clone()),
            ..ArtifactView::default()
        }
    }

    /// Draws the current detail cache: windowed when the lines are already
    /// wrapped at pane width (glow), wrap-and-scroll otherwise.
    fn render_cached_detail(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let viewport = usize::from(area.height.max(1));
        self.detail_viewport = viewport;
        let windowed = self
            .preview_cache
            .as_ref()
            .is_some_and(|cache| cache.windowed);
        if windowed {
            let total = self
                .preview_cache
                .as_ref()
                .map_or(0, |cache| cache.lines.len());
            let max_scroll = u16::try_from(total.saturating_sub(viewport)).unwrap_or(u16::MAX);
            self.detail_scroll = self.detail_scroll.min(max_scroll);
            let lines = self.preview_window(viewport);
            frame.render_widget(Paragraph::new(Text::from(lines)), area);
        } else {
            let total = self
                .preview_cache
                .as_ref()
                .map_or(0, |cache| wrapped_rows(&cache.lines, area.width.max(1)));
            let max_scroll = u16::try_from(total.saturating_sub(viewport)).unwrap_or(u16::MAX);
            self.detail_scroll = self.detail_scroll.min(max_scroll);
            let lines = self.preview_cache_lines();
            frame.render_widget(
                Paragraph::new(Text::from(lines))
                    .wrap(Wrap { trim: false })
                    .scroll((self.detail_scroll, 0)),
                area,
            );
        }
    }

    fn preview_window(&self, viewport: usize) -> Vec<Line<'static>> {
        let scroll = usize::from(self.detail_scroll);
        self.preview_cache.as_ref().map_or_else(Vec::new, |cache| {
            cache
                .lines
                .iter()
                .skip(scroll)
                .take(viewport)
                .cloned()
                .collect()
        })
    }

    fn prepare_artifact_detail_cache(
        &mut self,
        module_index: usize,
        artifact_index: usize,
        width: u16,
    ) {
        let cache_width = width.max(1);
        if self.detail_tab == DetailTab::Code {
            let module = &self.view.modules[module_index];
            let artifact = &module.artifacts[artifact_index];
            let key = format!("{}:{}", module.name, artifact.relative_path);
            let needs_build = self
                .code_cache
                .as_ref()
                .is_none_or(|cache| cache.path != key);
            if needs_build {
                self.detail_scroll = 0;
                self.code_cursor = 0;
                let lines = rich::highlight_code(&artifact.relative_path, &artifact.raw_source);
                self.code_cache = Some(CodeCache { path: key, lines });
                #[cfg(test)]
                {
                    self.code_cache_build_count += 1;
                }
            }
            return;
        }
        let key = {
            let module = &self.view.modules[module_index];
            let artifact = &module.artifacts[artifact_index];
            detail_cache_key(self.detail_tab, &module.name, &artifact.relative_path)
        };
        let needs_build = self
            .preview_cache
            .as_ref()
            .is_none_or(|cache| cache.key != key || cache.width != cache_width);
        if needs_build {
            // Preview and Diff spawn subprocesses (glow, git). While keys are
            // still queued — the user is holding j/k — keep the previous frame
            // and rebuild once input drains, so browsing never stutters.
            let expensive = matches!(self.detail_tab, DetailTab::Preview | DetailTab::Diff);
            if expensive && self.preview_cache.is_some() && input_pending() {
                return;
            }
            // A different target means new content: scrolling must restart at
            // the top, or a short document renders as a blank pane.
            if self
                .preview_cache
                .as_ref()
                .is_some_and(|cache| cache.key != key)
            {
                self.detail_scroll = 0;
            }
            let (lines, windowed) = {
                let module = &self.view.modules[module_index];
                let artifact = &module.artifacts[artifact_index];
                self.build_detail_lines(Some(module), artifact, self.detail_tab, cache_width)
            };
            let hunks = hunk_offsets(&lines);
            self.preview_cache = Some(DetailCache {
                key,
                width: cache_width,
                lines,
                windowed,
                hunks,
            });
            #[cfg(test)]
            {
                self.preview_cache_build_count += 1;
            }
        }
    }

    /// Renders one detail tab to lines: the single pipeline behind the detail
    /// pane and the fullscreen zoom, so both show the same rich content.
    fn build_detail_lines(
        &self,
        module: Option<&ModuleView>,
        artifact: &ArtifactView,
        tab: DetailTab,
        width: u16,
    ) -> (Vec<Line<'static>>, bool) {
        match tab {
            DetailTab::Preview => preview_lines_for_width(artifact, width),
            DetailTab::Code => (
                expand_gutter_wrapped(
                    rich::highlight_code(&artifact.relative_path, &artifact.raw_source),
                    CODE_GUTTER,
                    usize::from(width),
                ),
                true,
            ),
            DetailTab::Diff => (
                expand_gutter_wrapped(diff_lines(module, artifact, width), 1, usize::from(width)),
                true,
            ),
            DetailTab::Provenance => (
                expand_gutter_wrapped(
                    module.map_or_else(
                        || vec![Line::from("module not found")],
                        |module| self.provenance_lines(module, artifact),
                    ),
                    2,
                    usize::from(width),
                ),
                true,
            ),
            DetailTab::Frontmatter => (frontmatter_lines(artifact, width), true),
            DetailTab::History => (
                expand_gutter_wrapped(history_lines(artifact), 2, usize::from(width)),
                true,
            ),
            DetailTab::Companions => (
                expand_gutter_wrapped(companion_lines(artifact), 2, usize::from(width)),
                true,
            ),
        }
    }

    fn preview_cache_lines(&self) -> Vec<Line<'static>> {
        self.preview_cache
            .as_ref()
            .map_or_else(Vec::new, |cache| cache.lines.clone())
    }

    fn code_window(&self, artifact: &ArtifactView, viewport: usize) -> Vec<Line<'static>> {
        let scroll = usize::from(self.detail_scroll);
        let current_line = self.current_code_line(artifact);
        let module = &artifact.module;
        let path = &artifact.relative_path;
        self.code_cache.as_ref().map_or_else(Vec::new, |cache| {
            cache
                .lines
                .iter()
                .enumerate()
                .skip(scroll)
                .take(viewport)
                .map(|(index, cached_line)| {
                    let mut line = cached_line.clone();
                    let line_number = index + 1;
                    let has_comment =
                        self.comments
                            .contains_key(&(module.clone(), path.clone(), line_number));
                    if let Some(marker) = line.spans.first_mut() {
                        *marker = Span::styled(
                            if has_comment { "◆ " } else { "  " },
                            if has_comment {
                                Style::default().fg(Color::Yellow)
                            } else {
                                Style::default().fg(Color::DarkGray)
                            },
                        );
                    }
                    if line_number == current_line {
                        line.style = selected_style(self.focused == ColumnFocus::Detail);
                    }
                    line
                })
                .collect()
        })
    }

    fn render_tabs(&self, frame: &mut Frame<'_>, area: Rect) {
        let spans = DetailTab::ALL
            .iter()
            .flat_map(|tab| {
                let style = if *tab == self.detail_tab {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::DarkGray)
                };
                [Span::raw(" "), Span::styled(tab.label(), style)]
            })
            .collect::<Vec<_>>();
        frame.render_widget(Paragraph::new(Line::from(spans)), area);
    }

    fn render_overview_detail(&self, frame: &mut Frame<'_>, area: Rect) {
        let mut lines = vec![Line::from(Span::styled(
            "Status summary",
            Style::default().add_modifier(Modifier::BOLD),
        ))];
        let summary = &self.view.summary;
        lines.push(Line::from(format!(
            "unchanged {} · stale {} · modified {} · new {}",
            summary.unchanged, summary.stale, summary.modified, summary.new
        )));
        lines.push(Line::from(""));
        if self.overview_mode == OverviewMode::Matrix {
            let matrix = builders::build_matrix(&self.view);
            lines.push(Line::from(Span::styled(
                "Matrix",
                Style::default().fg(Color::Magenta),
            )));
            lines.push(Line::from(format!("columns: {}", matrix.cols.join(", "))));
            for row in matrix.rows {
                let cells = row
                    .cells
                    .iter()
                    .map(|cell| format!("{}:{}{}", cell.kind, cell.count, status_dot(&cell.status)))
                    .collect::<Vec<_>>()
                    .join("  ");
                lines.push(Line::from(format!("{}  {cells}", row.module)));
            }
        } else {
            lines.push(Line::from(Span::styled(
                "Nested",
                Style::default().fg(Color::Magenta),
            )));
            for group in builders::build_nested(&self.view, "kind") {
                lines.push(Line::from(format!("{} ({})", group.label, group.count)));
                for subgroup in group.subgroups {
                    lines.push(Line::from(format!(
                        "  {} ({})",
                        subgroup.label, subgroup.count
                    )));
                }
            }
        }
        frame.render_widget(
            Paragraph::new(Text::from(lines)).wrap(Wrap { trim: false }),
            area,
        );
    }

    fn render_variant_detail(
        &self,
        frame: &mut Frame<'_>,
        area: Rect,
        module: &str,
        kind: &str,
        name: &str,
        qualifier: &str,
    ) {
        let mut lines = vec![
            Line::from(Span::styled(
                format!("{module} / {kind} / {name}"),
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(format!("qualifier: {qualifier}")),
            Line::from(""),
        ];
        if let Some((_, artifact)) = self.find_artifact(module, kind, name)
            && let Some(variant) = artifact
                .variants
                .iter()
                .find(|variant| variant.qualifier == qualifier)
        {
            lines.push(Line::from(format!("merge mode: {}", variant.mode)));
            lines.push(Line::from(format!("path: {}", variant.relative_path)));
            lines.push(Line::from(""));
            lines.push(Line::from(
                "effective merge preview is deferred to the dashboard route",
            ));
        }
        frame.render_widget(
            Paragraph::new(Text::from(lines)).wrap(Wrap { trim: false }),
            area,
        );
    }

    pub fn request_quit(&mut self) {
        if !self.comments.is_empty() && !self.quit_armed {
            self.quit_armed = true;
            self.toast = Some(format!(
                "{} unsaved comments — press q again to quit (Y copies them first)",
                self.comments.len()
            ));
            return;
        }
        self.run_state = RunState::Quit;
    }

    pub fn disarm_quit(&mut self) {
        self.quit_armed = false;
    }

    /// Esc walks focus back toward Sections and quits only from there —
    /// backing out of a pane must never kill the session.
    pub fn escape(&mut self) {
        match self.focused {
            ColumnFocus::Detail | ColumnFocus::List => self.focus_previous(),
            ColumnFocus::Sections => self.request_quit(),
        }
    }

    #[must_use]
    pub fn should_quit(&self) -> bool {
        self.run_state == RunState::Quit
    }

    #[must_use]
    pub fn is_preview_open(&self) -> bool {
        self.preview.is_some()
    }

    #[must_use]
    pub fn is_help_open(&self) -> bool {
        self.help_state == HelpState::Open
    }

    pub fn toggle_help(&mut self) {
        self.help_state = match self.help_state {
            HelpState::Closed => HelpState::Open,
            HelpState::Open => HelpState::Closed,
        };
    }

    pub fn close_help(&mut self) {
        self.help_state = HelpState::Closed;
    }

    pub fn close_preview(&mut self) {
        self.preview = None;
    }

    pub fn preview_scroll_down(&mut self, amount: u16) {
        if let Some(preview) = self.preview.as_mut() {
            preview.scroll_down(amount);
        }
    }

    pub fn preview_scroll_up(&mut self, amount: u16) {
        if let Some(preview) = self.preview.as_mut() {
            preview.scroll_up(amount);
        }
    }

    pub fn preview_scroll_to_top(&mut self) {
        if let Some(preview) = self.preview.as_mut() {
            preview.scroll_to_top();
        }
    }

    pub fn preview_scroll_to_bottom(&mut self) {
        if let Some(preview) = self.preview.as_mut() {
            preview.scroll_to_bottom();
        }
    }

    #[must_use]
    pub fn is_palette_open(&self) -> bool {
        self.palette.is_open()
    }

    pub fn open_palette(&mut self) {
        self.palette_error = None;
        self.palette.open();
    }

    pub fn close_palette(&mut self) {
        self.palette.close();
    }

    pub fn palette_key(&mut self, key: KeyEvent) {
        let _ = self.palette.on_key(key);
    }

    pub fn execute_palette(&mut self) {
        let command = self.palette.take_command();
        self.execute_palette_command(command);
    }

    pub fn execute_palette_command(&mut self, command: PaletteCommand) {
        self.palette_error = None;
        match command {
            PaletteCommand::Refresh => self.refresh(),
            PaletteCommand::Quit => {
                self.request_quit();
            }
            PaletteCommand::Find(query) => {
                self.search.query = query;
                self.set_section(Section::Search);
                self.focused = ColumnFocus::List;
                self.list_selected[self.section as usize] = 0;
                self.invalidate_rows();
            }
            PaletteCommand::GoTo(section) => {
                if let Some(section) = Section::from_name(&section) {
                    self.set_section(section);
                } else {
                    self.palette_error = Some(format!("unknown section: {section}"));
                }
            }
            PaletteCommand::Sort(field) => {
                self.search.sort = field;
                self.set_section(Section::Search);
                self.invalidate_rows();
            }
            PaletteCommand::Filter(value) => {
                if matches!(value.as_str(), "skills" | "agents" | "rules") {
                    self.search.kind = value;
                } else {
                    self.search.status = value;
                }
                self.set_section(Section::Search);
                self.invalidate_rows();
            }
            PaletteCommand::Empty => {}
            PaletteCommand::Unknown(verb) => {
                self.palette_error = Some(format!("unknown command: {verb}"));
            }
        }
    }

    pub fn focus_next(&mut self) {
        self.focused = match self.focused {
            ColumnFocus::Sections => ColumnFocus::List,
            ColumnFocus::List | ColumnFocus::Detail => ColumnFocus::Detail,
        };
    }

    pub fn focus_previous(&mut self) {
        self.focused = match self.focused {
            ColumnFocus::Sections | ColumnFocus::List => ColumnFocus::Sections,
            ColumnFocus::Detail => ColumnFocus::List,
        };
    }

    /// One navigation step in the detail pane: moves the Code cursor when the
    /// Code tab is active, otherwise scrolls the viewport.
    fn detail_step(&mut self, delta: isize) {
        if self.detail_tab == DetailTab::Code {
            self.move_code_cursor(delta);
        } else if delta.is_negative() {
            self.detail_scroll = self
                .detail_scroll
                .saturating_sub(u16::try_from(-delta).unwrap_or(0));
        } else {
            self.detail_scroll = self
                .detail_scroll
                .saturating_add(u16::try_from(delta).unwrap_or(0));
        }
    }

    /// Jumps the diff viewport to the next or previous hunk header.
    fn jump_hunk(&mut self, forward: bool) {
        let Some(cache) = self.preview_cache.as_ref() else {
            return;
        };
        let current = usize::from(self.detail_scroll);
        let target = if forward {
            cache.hunks.iter().find(|&&offset| offset > current)
        } else {
            cache.hunks.iter().rev().find(|&&offset| offset < current)
        };
        if let Some(&offset) = target {
            self.detail_scroll = u16::try_from(offset).unwrap_or(u16::MAX);
        }
    }

    /// (current hunk, total hunks) for the footer while the Diff tab scrolls.
    fn hunk_position(&self) -> Option<(usize, usize)> {
        let cache = self.preview_cache.as_ref()?;
        if self.detail_tab != DetailTab::Diff || cache.hunks.is_empty() {
            return None;
        }
        let current = usize::from(self.detail_scroll);
        let index = cache
            .hunks
            .iter()
            .take_while(|&&offset| offset <= current)
            .count()
            .max(1);
        Some((index, cache.hunks.len()))
    }

    fn toggle_overview_mode(&mut self) {
        self.overview_mode = match self.overview_mode {
            OverviewMode::Nested => OverviewMode::Matrix,
            OverviewMode::Matrix => OverviewMode::Nested,
        };
        self.invalidate_rows();
    }

    pub fn drill_or_expand(&mut self) {
        self.ensure_rows();
        match self.focused {
            ColumnFocus::Sections => self.focused = ColumnFocus::List,
            ColumnFocus::List => {
                if let Some(ListTarget::OverviewMode) = self.selected_target() {
                    self.toggle_overview_mode();
                    return;
                }
                if let Some(ListTarget::ProvenanceArtifact { .. }) = self.selected_target() {
                    self.detail_tab = DetailTab::Provenance;
                }
                self.focused = ColumnFocus::Detail;
                self.detail_scroll = 0;
            }
            ColumnFocus::Detail => {
                if let Some(artifact) = self.selected_artifact().cloned() {
                    self.preview = Some(ArtifactPreview::from_artifact(&artifact));
                }
            }
        }
    }

    pub fn move_back(&mut self) {
        self.focus_previous();
    }

    /// Left click: focus the pane under the cursor; select the section, list
    /// row, or detail tab it lands on. Clicks are discrete and idempotent, so
    /// mapping them to selection is safe (unlike wheel events).
    pub fn mouse_click(&mut self, x: u16, y: u16) {
        if self.preview.is_some()
            || self.help_state == HelpState::Open
            || self.palette.is_open()
            || self.comment_prompt.is_some()
        {
            return;
        }
        let position = Position { x, y };
        let regions = self.mouse_regions;
        if regions.tabs.contains(position) {
            self.focused = ColumnFocus::Detail;
            if y == regions.tabs.y
                && let Some(tab) = tab_at_column(x.saturating_sub(regions.tabs.x))
            {
                self.set_detail_tab(tab);
            }
        } else if regions.sections.contains(position) {
            self.focused = ColumnFocus::Sections;
            if let Some(row) = bordered_row_at(regions.sections, x, y)
                && row < Section::ALL.len()
            {
                self.set_section(Section::ALL[row]);
            }
        } else if regions.list.contains(position) {
            self.focused = ColumnFocus::List;
            if let Some(visual_row) = bordered_row_at(regions.list, x, y) {
                let row = visual_row.saturating_add(self.list_offset);
                self.ensure_rows();
                let rows = self.cached_rows();
                let selectable = rows.get(row).is_some_and(ListRow::is_selectable);
                let toggles = rows
                    .get(row)
                    .is_some_and(|hit| matches!(hit.target, ListTarget::OverviewMode));
                if selectable {
                    let already_selected = self.list_selected[self.section as usize] == row;
                    self.list_selected[self.section as usize] = row;
                    if toggles {
                        self.toggle_overview_mode();
                    } else if already_selected {
                        // Click on the selected row activates it, gitui-style.
                        self.drill_or_expand();
                    }
                }
            }
        } else if regions.detail.contains(position) {
            self.focused = ColumnFocus::Detail;
        }
    }

    /// Mouse wheel scrolls the viewport under the cursor and never moves the
    /// selection: passive trackpad gestures must not drag application state.
    pub fn mouse_scroll(&mut self, x: u16, y: u16, down: bool) {
        const WHEEL_STEP: u16 = 3;
        if self.preview.is_some() {
            if down {
                self.preview_scroll_down(WHEEL_STEP);
            } else {
                self.preview_scroll_up(WHEEL_STEP);
            }
            return;
        }
        if self.help_state == HelpState::Open {
            return;
        }
        let position = Position { x, y };
        if self.mouse_regions.detail.contains(position) {
            self.detail_scroll = if down {
                self.detail_scroll.saturating_add(WHEEL_STEP)
            } else {
                self.detail_scroll.saturating_sub(WHEEL_STEP)
            };
        } else if self.mouse_regions.list.contains(position) {
            // Viewport only; the render pass clamps to the row count.
            self.list_offset = if down {
                self.list_offset.saturating_add(usize::from(WHEEL_STEP))
            } else {
                self.list_offset.saturating_sub(usize::from(WHEEL_STEP))
            };
        }
    }

    pub fn focused_key(&mut self, key: KeyEvent) {
        match self.focused {
            ColumnFocus::Sections => self.section_key(key),
            ColumnFocus::List => self.list_key(key),
            ColumnFocus::Detail => self.detail_key(key),
        }
    }

    pub fn set_section_by_number(&mut self, number: usize) {
        if (1..=SECTION_COUNT).contains(&number) {
            self.set_section(Section::from_index(number - 1));
        }
    }

    /// Toasts show until the next keypress, then yield the footer back to the
    /// hint row.
    pub fn clear_toast(&mut self) {
        self.toast = None;
    }

    pub fn set_toast(&mut self, message: String) {
        self.toast = Some(message);
    }

    /// Queue gitui (or jjui) for the selected repository; the event loop
    /// suspends the TUI, runs the tool in the repo, and resumes on exit.
    pub fn open_repo_tool(&mut self, jj: bool) {
        let program = if jj { "jjui" } else { "gitui" };
        let name = match self.selected_target() {
            Some(ListTarget::Module(name)) => name,
            Some(
                ListTarget::Artifact { module, .. }
                | ListTarget::ProvenanceArtifact { module, .. }
                | ListTarget::Companion { module, .. },
            ) => module,
            Some(ListTarget::Adr { repo, .. }) => repo,
            _ => {
                self.toast = Some(format!("{program}: select a repository or artifact first"));
                return;
            }
        };
        let Some(module) = self.view.modules.iter().find(|module| module.name == name) else {
            return;
        };
        let Some(path) = module.local_path.clone() else {
            self.toast = Some(format!("{program}: no local clone for {name}"));
            return;
        };
        self.pending_external = Some((program.to_string(), path));
    }

    pub fn take_external(&mut self) -> Option<(String, PathBuf)> {
        self.pending_external.take()
    }

    pub fn set_section_by_shortcut(&mut self, character: char) -> bool {
        let Some(section) = Section::from_shortcut(character) else {
            return false;
        };
        self.set_section(section);
        true
    }

    pub fn set_detail_tab(&mut self, tab: DetailTab) {
        if self.detail_tab == tab {
            return;
        }
        self.detail_tab = tab;
        self.detail_scroll = 0;
    }

    #[cfg(test)]
    #[must_use]
    pub fn detail_tab(&self) -> DetailTab {
        self.detail_tab
    }

    #[cfg(test)]
    #[must_use]
    pub fn focused_column(&self) -> ColumnFocus {
        self.focused
    }

    #[cfg(test)]
    #[must_use]
    pub fn detail_scroll_for_test(&self) -> u16 {
        self.detail_scroll
    }

    #[cfg(test)]
    #[must_use]
    pub fn selected_row_for_test(&self) -> usize {
        self.list_selected[self.section as usize]
    }

    #[cfg(test)]
    #[must_use]
    pub fn section(&self) -> Section {
        self.section
    }

    #[cfg(test)]
    #[must_use]
    pub fn search_query(&self) -> &str {
        &self.search.query
    }

    #[must_use]
    pub fn has_section_digit_shortcuts(&self) -> bool {
        self.focused == ColumnFocus::Sections
    }

    #[must_use]
    pub fn is_search_input_active(&self) -> bool {
        self.section == Section::Search && self.focused == ColumnFocus::List && self.search_typing
    }

    pub fn begin_search_input(&mut self) {
        self.focused = ColumnFocus::List;
        self.search_typing = true;
    }

    pub fn search_input_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Enter => {
                self.search_typing = false;
                self.clamp_list_selection();
            }
            KeyCode::Backspace => {
                self.search.query.pop();
                self.list_selected[self.section as usize] = 0;
                self.invalidate_rows();
            }
            KeyCode::Char(character) => {
                self.search.query.push(character);
                self.list_selected[self.section as usize] = 0;
                self.invalidate_rows();
            }
            _ => {}
        }
    }

    #[must_use]
    pub fn is_comment_prompt_open(&self) -> bool {
        self.comment_prompt.is_some()
    }

    pub fn comment_prompt_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.comment_prompt = None,
            KeyCode::Tab => {
                if let Some(prompt) = self.comment_prompt.as_mut() {
                    prompt.kind = prompt.kind.next();
                }
            }
            KeyCode::Enter => self.save_comment_prompt(),
            KeyCode::Backspace => {
                if let Some(prompt) = self.comment_prompt.as_mut() {
                    prompt.text.pop();
                }
            }
            KeyCode::Char(character) => {
                if let Some(prompt) = self.comment_prompt.as_mut() {
                    prompt.text.push(character);
                }
            }
            _ => {}
        }
    }

    #[cfg(test)]
    #[must_use]
    pub fn row_build_count(&self) -> usize {
        self.row_build_count
    }

    pub fn copy_selected(&mut self) {
        self.ensure_rows();
        if let Some(path) = self
            .selected_artifact()
            .map(|artifact| artifact.relative_path.clone())
        {
            self.toast = Some(if copy_to_pbcopy(&path) {
                format!("copied source path: {path}")
            } else {
                "pbcopy unavailable".to_string()
            });
        }
    }

    pub fn copy_tuicr_review(&mut self) {
        if self.comments.is_empty() {
            self.toast = Some("no comments to copy".to_string());
            return;
        }

        let digest = self.tuicr_digest();
        let copied = copy_to_pbcopy(&digest);
        let count = self.comments.len();
        self.toast = Some(if copied {
            format!("copied {count} comments")
        } else {
            // stderr is invisible inside the alternate screen; a file is the
            // only fallback that survives.
            let fallback = std::env::temp_dir().join("forge-tuicr-review.md");
            match std::fs::write(&fallback, &digest) {
                Ok(()) => format!(
                    "pbcopy unavailable — review written to {}",
                    fallback.display()
                ),
                Err(error) => format!("pbcopy unavailable and file write failed: {error}"),
            }
        });
    }

    #[cfg(test)]
    pub fn add_comment_for_test(
        &mut self,
        module: impl Into<String>,
        path: impl Into<String>,
        line_number: usize,
        kind: CommentKind,
        text: impl Into<String>,
    ) {
        self.comments.insert(
            (module.into(), path.into(), line_number),
            LineComment {
                kind,
                text: text.into(),
            },
        );
    }

    #[must_use]
    pub fn tuicr_digest(&self) -> String {
        let mut lines = vec![
            "I reviewed your code and have the following comments. Please address them."
                .to_string(),
            String::new(),
        ];
        lines.extend(self.comments.iter().enumerate().map(
            |(index, ((module, path, line_number), comment))| {
                format!(
                    "{}. **[{}]** `{}:{}` ({}) - {}",
                    index + 1,
                    comment.kind.label(),
                    path,
                    line_number,
                    module,
                    comment.text
                )
            },
        ));
        lines.join("\n")
    }

    fn open_comment_prompt(&mut self) {
        let Some(artifact) = self.selected_artifact() else {
            return;
        };
        let module = artifact.module.clone();
        let path = artifact.relative_path.clone();
        let line_number = self.current_code_line(artifact);
        let (kind, text) = self
            .comments
            .get(&(module.clone(), path.clone(), line_number))
            .map_or((CommentKind::Issue, String::new()), |comment| {
                (comment.kind, comment.text.clone())
            });
        self.comment_prompt = Some(CommentPrompt {
            module,
            path,
            line_number,
            kind,
            text,
        });
    }

    fn save_comment_prompt(&mut self) {
        let Some(prompt) = self.comment_prompt.take() else {
            return;
        };
        let text = prompt.text.trim().to_string();
        if text.is_empty() {
            self.comments
                .remove(&(prompt.module, prompt.path, prompt.line_number));
            self.toast = Some("comment cleared".to_string());
            return;
        }
        self.comments.insert(
            (prompt.module, prompt.path, prompt.line_number),
            LineComment {
                kind: prompt.kind,
                text,
            },
        );
        self.toast = Some("comment saved".to_string());
    }

    fn section_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Down | KeyCode::Char('j') => {
                let next = (self.section as usize + 1).min(SECTION_COUNT - 1);
                self.set_section(Section::from_index(next));
            }
            KeyCode::Up | KeyCode::Char('k') => {
                let next = (self.section as usize).saturating_sub(1);
                self.set_section(Section::from_index(next));
            }
            KeyCode::Home | KeyCode::Char('g') => self.set_section(Section::Overview),
            KeyCode::End | KeyCode::Char('G') => self.set_section(Section::Schemas),
            _ => {}
        }
    }

    fn list_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Down | KeyCode::Char('j') => self.move_list_selection(1),
            KeyCode::Up | KeyCode::Char('k') => self.move_list_selection(-1),
            KeyCode::Home | KeyCode::Char('g') => self.select_first_row(),
            KeyCode::End | KeyCode::Char('G') => self.select_last_row(),
            KeyCode::Char('m') if self.section == Section::Overview => {
                self.toggle_overview_mode();
            }
            KeyCode::Char('m') if self.selected_artifact().is_some() => {
                self.focused = ColumnFocus::Detail;
                self.set_detail_tab(DetailTab::Code);
                self.open_comment_prompt();
            }
            _ => {}
        }
    }

    fn detail_key(&mut self, key: KeyEvent) {
        let page = isize::try_from(self.detail_viewport.max(2) - 1).unwrap_or(10);
        let half = (page / 2).max(1);
        match key.code {
            KeyCode::Down | KeyCode::Char('j') => self.detail_step(1),
            KeyCode::Up | KeyCode::Char('k') => self.detail_step(-1),
            KeyCode::PageDown | KeyCode::Char(' ') => self.detail_step(page),
            KeyCode::PageUp | KeyCode::Char('b') => self.detail_step(-page),
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.detail_step(half);
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.detail_step(-half);
            }
            KeyCode::Home | KeyCode::Char('g') => {
                self.code_cursor = 0;
                self.detail_scroll = 0;
            }
            KeyCode::End | KeyCode::Char('G') => {
                self.code_cursor = usize::MAX;
                self.detail_scroll = u16::MAX;
                self.move_code_cursor(0);
            }
            KeyCode::Char(']') if self.detail_tab == DetailTab::Diff => self.jump_hunk(true),
            KeyCode::Char('[') if self.detail_tab == DetailTab::Diff => self.jump_hunk(false),
            KeyCode::Char('p') => self.set_detail_tab(DetailTab::Preview),
            KeyCode::Char('c') => self.set_detail_tab(DetailTab::Code),
            KeyCode::Char('d') => self.set_detail_tab(DetailTab::Diff),
            KeyCode::Char('v') => self.set_detail_tab(DetailTab::Provenance),
            KeyCode::Char('f') => self.set_detail_tab(DetailTab::Frontmatter),
            KeyCode::Char('i') => self.set_detail_tab(DetailTab::History),
            KeyCode::Char('n') => self.set_detail_tab(DetailTab::Companions),
            KeyCode::Tab => self.next_detail_tab(),
            KeyCode::Char('m') => {
                if self.detail_tab != DetailTab::Code {
                    self.set_detail_tab(DetailTab::Code);
                }
                self.open_comment_prompt();
            }
            _ => {}
        }
    }

    fn next_detail_tab(&mut self) {
        let next = (self.detail_tab as usize + 1) % DETAIL_TAB_COUNT;
        self.set_detail_tab(DetailTab::from_index(next));
    }

    fn set_section(&mut self, section: Section) {
        self.section = section;
        self.detail_scroll = 0;
        self.list_offset = 0;
        self.invalidate_rows();
        self.clamp_list_selection();
    }

    fn ensure_rows(&mut self) {
        if self.rows_dirty {
            self.cached_rows = self.build_list_rows();
            self.column_widths = column_widths_for_rows(&self.cached_rows);
            self.rows_dirty = false;
            #[cfg(test)]
            {
                self.row_build_count += 1;
            }
        }
    }

    fn invalidate_rows(&mut self) {
        self.rows_dirty = true;
    }

    fn invalidate_detail_caches(&mut self) {
        self.preview_cache = None;
        self.code_cache = None;
    }

    /// After a rescan the zoom overlay's cloned artifact is stale: rebind it
    /// to the fresh view, or close it when the artifact no longer exists.
    fn refresh_open_preview(&mut self) {
        let Some((open, scroll)) = self.preview.as_ref().map(|preview| {
            let artifact = preview.artifact();
            (
                (
                    artifact.module.clone(),
                    artifact.kind.clone(),
                    artifact.name.clone(),
                ),
                preview.scroll(),
            )
        }) else {
            return;
        };
        let fresh = self
            .view
            .modules
            .iter()
            .find(|module| module.name == open.0)
            .and_then(|module| {
                module
                    .artifacts
                    .iter()
                    .find(|artifact| artifact.kind == open.1 && artifact.name == open.2)
            })
            .cloned();
        self.preview = fresh.map(|artifact| {
            let mut preview = ArtifactPreview::from_artifact(&artifact);
            preview.scroll_down(scroll);
            preview
        });
    }

    fn cached_rows(&self) -> &[ListRow] {
        &self.cached_rows
    }

    #[cfg(test)]
    #[must_use]
    pub fn column_widths_for_total(&mut self, total_width: u16) -> MillerColumnWidths {
        self.ensure_rows();
        fit_miller_widths(total_width, self.column_widths)
    }

    #[cfg(test)]
    #[must_use]
    pub fn preview_cache_build_count(&self) -> usize {
        self.preview_cache_build_count
    }

    #[cfg(test)]
    #[must_use]
    pub fn code_cache_build_count(&self) -> usize {
        self.code_cache_build_count
    }

    fn build_list_rows(&self) -> Vec<ListRow> {
        match self.section {
            Section::Overview => self.overview_rows(),
            Section::Skills => self.artifact_rows(Some("skills")),
            Section::Agents => self.artifact_rows(Some("agents")),
            Section::Rules => self.artifact_rows(Some("rules")),
            Section::Repositories => self.repository_rows(),
            Section::Adrs => self.adr_rows(),
            Section::Provenance => self.provenance_rows(),
            Section::Variants => self.variant_rows(),
            Section::Search => self.search_rows(),
            Section::Settings => self.settings_rows(),
            Section::Hooks => self.hook_rows(),
            Section::Config => self.config_rows(),
            Section::Schemas => self.schema_rows(),
        }
    }

    fn overview_rows(&self) -> Vec<ListRow> {
        vec![
            ListRow::item("Summary", "status counts", ListTarget::Overview, "ok"),
            ListRow::item(
                if self.overview_mode == OverviewMode::Matrix {
                    "Matrix view"
                } else {
                    "Nested view"
                },
                "Enter or click toggles",
                ListTarget::OverviewMode,
                "ok",
            ),
        ]
    }

    fn artifact_rows(&self, kind_filter: Option<&str>) -> Vec<ListRow> {
        let mut rows = Vec::new();
        for (kind, artifacts) in self.view.artifacts_by_kind() {
            if kind_filter.is_some_and(|filter| filter != kind) {
                continue;
            }
            rows.push(ListRow::header(kind));
            for (artifact, module) in artifacts {
                rows.push(artifact_row(artifact, module));
                for companion in &artifact.companions {
                    rows.push(ListRow::item(
                        format!("  ↳ {}", companion.name),
                        format!("companion of {}", artifact.name),
                        ListTarget::Companion {
                            module: module.to_string(),
                            parent: artifact.name.clone(),
                            name: companion.name.clone(),
                        },
                        "ok",
                    ));
                }
            }
        }
        rows
    }

    fn repository_rows(&self) -> Vec<ListRow> {
        let mut rows = vec![ListRow::header("Sources")];
        for module in self.view.source_modules() {
            rows.push(ListRow::item(
                module.name.clone(),
                format!("{} artifacts", module.artifacts.len()),
                ListTarget::Module(module.name.clone()),
                "source",
            ));
        }
        rows.push(ListRow::header("Targets"));
        for module in self.view.target_modules() {
            rows.push(ListRow::item(
                module.name.clone(),
                format!("{} artifacts", module.artifacts.len()),
                ListTarget::Module(module.name.clone()),
                "new",
            ));
        }
        rows
    }

    fn adr_rows(&self) -> Vec<ListRow> {
        let mut rows = Vec::new();
        for repo in self.view.adrs_grouped() {
            rows.push(ListRow::header(format!("{} ({})", repo.repo, repo.total)));
            for group in repo.prefix_groups {
                rows.push(ListRow::header(format!("  {}", group.prefix)));
                for adr in group.adrs {
                    rows.push(ListRow::item(
                        format!("{} {}", adr.id, adr.title),
                        format!("{} · {}", adr.state, adr.summary),
                        ListTarget::Adr {
                            repo: adr.repo.clone(),
                            id: adr.id.clone(),
                        },
                        "source",
                    ));
                }
            }
        }
        rows
    }

    fn provenance_rows(&self) -> Vec<ListRow> {
        let mut rows = vec![ListRow::header("Needs attention")];
        for module in &self.view.modules {
            for artifact in &module.artifacts {
                let status = artifact.overall_status();
                if matches!(status, "modified" | "stale") || artifact.has_broken_refs() {
                    rows.push(ListRow::item(
                        artifact.name.clone(),
                        format!("{} · {}", artifact.kind, artifact.staleness_label()),
                        ListTarget::ProvenanceArtifact {
                            module: module.name.clone(),
                            kind: artifact.kind.clone(),
                            name: artifact.name.clone(),
                        },
                        status,
                    ));
                }
            }
        }
        if rows.len() == 1 {
            rows.push(ListRow::item(
                "No attention items",
                "integrity clean",
                ListTarget::Overview,
                "ok",
            ));
        }
        rows
    }

    fn variant_rows(&self) -> Vec<ListRow> {
        let coverage = builders::build_variant_coverage(&self.view);
        let mut rows = Vec::new();
        rows.push(ListRow::header(format!(
            "{} qualifiers",
            coverage.cols.len()
        )));
        for row in coverage.rows {
            for (index, cell) in row.cells.iter().enumerate() {
                if cell.mode.is_empty() {
                    continue;
                }
                let qualifier = coverage.cols[index].qualifier.clone();
                rows.push(ListRow::item(
                    row.name.clone(),
                    format!("{} · {} · {}", row.kind, qualifier, cell.mode),
                    ListTarget::Variant {
                        module: row.module.clone(),
                        kind: row.kind.clone(),
                        name: row.name.clone(),
                        qualifier,
                    },
                    "source",
                ));
            }
        }
        rows
    }

    fn search_rows(&self) -> Vec<ListRow> {
        let mut rows = vec![ListRow::header(format!(
            "query: {}{}  kind: {}  status: {}  sort: {}",
            value_or_any(&self.search.query),
            if self.search_typing {
                "▌ (Enter done)"
            } else {
                "  (/ edits)"
            },
            value_or_any(&self.search.kind),
            value_or_any(&self.search.status),
            value_or_any(&self.search.sort)
        ))];
        for (artifact, module) in builders::search_results(&self.view, &self.search) {
            rows.push(artifact_row(artifact, module));
        }
        rows
    }

    fn settings_rows(&self) -> Vec<ListRow> {
        let mut rows = Vec::new();
        for (group_index, group) in self.file_sections.settings.iter().enumerate() {
            rows.push(ListRow::header(group.harness.clone()));
            for (file_index, file) in group.files.iter().enumerate() {
                rows.push(ListRow::item(
                    format!("{} · {}", group.harness, file.label),
                    file.path.clone(),
                    ListTarget::SettingsFile {
                        group: group_index,
                        index: file_index,
                    },
                    "source",
                ));
            }
        }
        rows
    }

    fn hook_rows(&self) -> Vec<ListRow> {
        let mut rows = Vec::new();
        for (group_index, group) in self.file_sections.hooks.iter().enumerate() {
            rows.push(ListRow::header(group.harness.clone()));
            for (hook_index, hook) in group.hooks.iter().enumerate() {
                let (_, command) = files::unwrap_shell(&hook.command);
                rows.push(ListRow::item(
                    format!("{} · {}", hook.event, value_or_any(&hook.matcher)),
                    command,
                    ListTarget::Hook {
                        group: group_index,
                        index: hook_index,
                    },
                    "source",
                ));
            }
        }
        rows
    }

    fn config_rows(&self) -> Vec<ListRow> {
        self.file_sections
            .config
            .iter()
            .enumerate()
            .map(|(index, file)| {
                ListRow::item(
                    file.path.clone(),
                    file.label.clone(),
                    ListTarget::ConfigFile(index),
                    "source",
                )
            })
            .collect()
    }

    fn schema_rows(&self) -> Vec<ListRow> {
        let mut rows = Vec::new();
        for (group_index, group) in self.file_sections.schemas.iter().enumerate() {
            rows.push(ListRow::header(group.source.clone()));
            for (file_index, file) in group.files.iter().enumerate() {
                rows.push(ListRow::item(
                    format!("{} · {}", file.label, group.source),
                    file.path.clone(),
                    ListTarget::SchemaFile {
                        group: group_index,
                        index: file_index,
                    },
                    "source",
                ));
            }
        }
        rows
    }

    fn selected_list_index(&self, rows: &[ListRow]) -> usize {
        let selected = self.list_selected[self.section as usize];
        if rows.get(selected).is_some_and(ListRow::is_selectable) {
            return selected;
        }
        rows.iter()
            .position(ListRow::is_selectable)
            .unwrap_or_default()
    }

    /// Re-selects the row carrying the same target after the rows were
    /// rebuilt, so a background rescan does not silently move the selection
    /// to whatever row now occupies the old index.
    fn restore_selection(&mut self, target: Option<ListTarget>) {
        let Some(target) = target else {
            return;
        };
        self.ensure_rows();
        if let Some(index) = self.cached_rows.iter().position(|row| row.target == target) {
            self.list_selected[self.section as usize] = index;
        }
    }

    fn clamp_list_selection(&mut self) {
        self.ensure_rows();
        let rows = self.cached_rows();
        let index = self.selected_list_index(rows);
        self.list_selected[self.section as usize] = index;
    }

    pub fn move_list_selection(&mut self, delta: isize) {
        self.ensure_rows();
        let rows = self.cached_rows();
        if rows.is_empty() {
            self.list_selected[self.section as usize] = 0;
            return;
        }
        let mut index = self.selected_list_index(rows);
        loop {
            let next = if delta.is_negative() {
                index.checked_sub(1)
            } else {
                (index + 1 < rows.len()).then_some(index + 1)
            };
            let Some(next) = next else {
                break;
            };
            index = next;
            if rows[index].is_selectable() {
                break;
            }
        }
        self.list_selected[self.section as usize] = index;
    }

    fn select_first_row(&mut self) {
        self.ensure_rows();
        let rows = self.cached_rows();
        self.list_selected[self.section as usize] = rows
            .iter()
            .position(ListRow::is_selectable)
            .unwrap_or_default();
    }

    fn select_last_row(&mut self) {
        self.ensure_rows();
        let rows = self.cached_rows();
        self.list_selected[self.section as usize] = rows
            .iter()
            .rposition(ListRow::is_selectable)
            .unwrap_or_default();
    }

    fn selected_target(&self) -> Option<ListTarget> {
        let rows = self.cached_rows();
        let selected = self.selected_list_index(rows);
        rows.get(selected).map(|row| row.target.clone())
    }

    fn selected_artifact(&self) -> Option<&ArtifactView> {
        match self.selected_target()? {
            ListTarget::Artifact { module, kind, name }
            | ListTarget::ProvenanceArtifact { module, kind, name } => self
                .find_artifact(&module, &kind, &name)
                .map(|(_, artifact)| artifact),
            _ => None,
        }
    }

    fn current_code_line(&self, artifact: &ArtifactView) -> usize {
        let line_count = artifact.raw_source.lines().count().max(1);
        self.code_cursor.saturating_add(1).min(line_count)
    }

    /// Moves the Code cursor and drags the viewport along only when the
    /// cursor leaves it.
    fn move_code_cursor(&mut self, delta: isize) {
        let total = self
            .code_cache
            .as_ref()
            .map_or(1, |cache| cache.lines.len().max(1));
        let cursor = self.code_cursor.saturating_add_signed(delta).min(total - 1);
        self.code_cursor = cursor;
        let scroll = usize::from(self.detail_scroll);
        let viewport = self.detail_viewport.max(1);
        if cursor < scroll {
            self.detail_scroll = u16::try_from(cursor).unwrap_or(u16::MAX);
        } else if cursor >= scroll + viewport {
            self.detail_scroll = u16::try_from(cursor + 1 - viewport).unwrap_or(u16::MAX);
        }
    }

    fn find_artifact(
        &self,
        module: &str,
        kind: &str,
        name: &str,
    ) -> Option<(&ModuleView, &ArtifactView)> {
        self.view
            .modules
            .iter()
            .find(|candidate| candidate.name == module)
            .and_then(|module_view| {
                module_view
                    .artifacts
                    .iter()
                    .find(|artifact| artifact.kind == kind && artifact.name == name)
                    .map(|artifact| (module_view, artifact))
            })
    }

    fn find_artifact_indices(
        &self,
        module: &str,
        kind: &str,
        name: &str,
    ) -> Option<(usize, usize)> {
        self.view
            .modules
            .iter()
            .enumerate()
            .find(|(_, candidate)| candidate.name == module)
            .and_then(|(module_index, module_view)| {
                module_view
                    .artifacts
                    .iter()
                    .position(|artifact| artifact.kind == kind && artifact.name == name)
                    .map(|artifact_index| (module_index, artifact_index))
            })
    }

    fn find_adr(&self, repo: &str, id: &str) -> Option<&Adr> {
        self.view
            .adrs
            .iter()
            .find(|adr| adr.repo == repo && adr.id == id)
    }

    fn settings_file(&self, group: usize, index: usize) -> Option<&files::ConfigFile> {
        self.file_sections
            .settings
            .get(group)
            .and_then(|group| group.files.get(index))
    }

    fn hook_entry(&self, group: usize, index: usize) -> Option<&files::HookEntry> {
        self.file_sections
            .hooks
            .get(group)
            .and_then(|group| group.hooks.get(index))
    }

    fn schema_file(&self, group: usize, index: usize) -> Option<&files::ConfigFile> {
        self.file_sections
            .schemas
            .get(group)
            .and_then(|group| group.files.get(index))
    }

    fn provenance_entries<'a>(
        &'a self,
        module: &ModuleView,
        artifact: &ArtifactView,
    ) -> Vec<&'a ProvenanceArtifact> {
        self.view
            .provenance
            .iter()
            .filter(|record| {
                canonical_source(&record.source_uri) == canonical_source(&module.source_uri)
            })
            .flat_map(|record| record.artifacts.iter())
            .filter(|entry| entry.source_path.ends_with(&artifact.relative_path))
            .collect()
    }

    fn provenance_lines(&self, module: &ModuleView, artifact: &ArtifactView) -> Vec<Line<'static>> {
        fn field(key: &str, value: String) -> Line<'static> {
            Line::from(vec![
                Span::styled(format!("{key:<14}"), Style::default().fg(Color::Magenta)),
                Span::raw(value),
            ])
        }
        fn field_if(key: &str, value: &str) -> Option<Line<'static>> {
            (!value.trim().is_empty()).then(|| field(key, value.to_string()))
        }
        let short = |sha: &str| sha.chars().take(12).collect::<String>();

        let mut lines = vec![
            Line::from(Span::styled(
                "Provenance",
                Style::default().add_modifier(Modifier::BOLD),
            )),
            field(
                "status",
                format!(
                    "{} · {}",
                    artifact.overall_status(),
                    artifact.staleness_label()
                ),
            ),
        ];
        if let Some(adoption) = &artifact.adoption {
            lines.push(field(
                "upstream",
                format!(
                    "{} @ {}",
                    adoption.source_label,
                    short(&adoption.source_sha)
                ),
            ));
            lines.push(field("adopted", adoption.kind.clone()));
            lines.extend(field_if("author", &adoption.author));
            if !adoption.dependencies.is_empty() {
                let deps = builders::resolve_dep_links(&self.view, artifact.adoption.as_ref())
                    .iter()
                    .map(|dep| {
                        if dep.module.is_empty() {
                            dep.name.clone()
                        } else {
                            format!("{} ({})", dep.name, dep.module)
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                lines.push(field("depends on", deps));
            }
            if !adoption.transforms.is_empty() {
                lines.push(field("transforms", adoption.transforms.join(", ")));
            }
            lines.extend(field_if("license", &adoption.license));
            lines.extend(field_if("adopted by", &adoption.adopted_by));
        } else {
            lines.push(field("upstream", "authored here".to_string()));
        }
        lines.push(field("source", module.name.clone()));

        let entries = self.provenance_entries(module, artifact);
        let groups = builders::group_deployments(&entries);
        lines.push(Line::default());
        lines.extend(deployment_lines(&groups));
        if !artifact.sidecar_warning.is_empty() {
            lines.push(field("sidecar", artifact.sidecar_warning.clone()));
        }
        lines.extend(sidecar_yaml_lines(module, artifact));
        lines
    }
}

/// The raw adoption sidecar, syntax-highlighted as YAML, appended to the
/// provenance chain when the file exists next to the source.
fn sidecar_yaml_lines(module: &ModuleView, artifact: &ArtifactView) -> Vec<Line<'static>> {
    let Some(repo) = module.local_path.as_ref() else {
        return Vec::new();
    };
    let source = if artifact.source_path.is_empty() {
        artifact.relative_path.as_str()
    } else {
        artifact.source_path.as_str()
    };
    let sidecar = Path::new(source).with_extension("yaml");
    let Ok(content) = std::fs::read_to_string(repo.join(&sidecar)) else {
        return Vec::new();
    };
    let mut lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            format!("Sidecar · {}", sidecar.display()),
            Style::default().add_modifier(Modifier::BOLD),
        )),
    ];
    lines.extend(rich::highlight_code(
        &sidecar.to_string_lossy(),
        content.trim_end(),
    ));
    lines
}

fn render_module_detail(frame: &mut Frame<'_>, area: Rect, module: &ModuleView, scroll: u16) {
    let mut lines = vec![
        Line::from(Span::styled(
            module.name.clone(),
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(format!("version: {}", module.version)),
        Line::from(format!("source: {}", module.source_uri)),
        Line::from(format!(
            "role: {}",
            if module.is_target { "target" } else { "source" }
        )),
    ];
    if let Some(local_path) = &module.local_path {
        lines.push(Line::from(format!("local: {}", local_path.display())));
    }
    if let Some(vcs) = &module.vcs {
        lines.push(module_vcs_line(vcs));
    }
    if !module.description.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(module.description.clone()));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(format!("artifacts: {}", module.artifacts.len())));
    if !module.git_log.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Recent commits",
            Style::default().add_modifier(Modifier::BOLD),
        )));
        for commit in &module.git_log {
            let sha_short: String = commit.sha.chars().take(7).collect();
            let date: String = commit.date.chars().take(10).collect();
            let mut spans = vec![
                Span::styled(
                    format!("{sha_short} {date} "),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::raw(commit.message.clone()),
            ];
            if !commit.jj_change.is_empty() {
                spans.push(Span::styled(
                    format!(" · jj {}", commit.jj_change),
                    Style::default().fg(Color::DarkGray),
                ));
            }
            lines.push(Line::from(spans));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "o open gitui · O open jjui",
            Style::default().fg(Color::DarkGray),
        )));
    }
    frame.render_widget(
        Paragraph::new(Text::from(lines))
            .wrap(Wrap { trim: false })
            .scroll((scroll, 0)),
        area,
    );
}

fn module_vcs_line(vcs: &VcsState) -> Line<'static> {
    use std::fmt::Write as _;
    let mut branch = vcs.branch.clone();
    if vcs.ahead > 0 {
        let _ = write!(branch, " ↑{}", vcs.ahead);
    }
    if vcs.behind > 0 {
        let _ = write!(branch, " ↓{}", vcs.behind);
    }
    if vcs.jj_colocated {
        branch.push_str(" · jj");
    }
    let (state_label, state_style) = match vcs.worktree {
        WorktreeState::Clean => ("✓ clean", Style::default().fg(Color::Green)),
        WorktreeState::Modified => ("⚠ uncommitted changes", Style::default().fg(Color::Yellow)),
        WorktreeState::Untracked => ("● untracked", Style::default().fg(Color::Magenta)),
    };
    Line::from(vec![
        Span::styled(branch, Style::default().fg(Color::Cyan)),
        Span::raw(" · "),
        Span::styled(state_label, state_style),
    ])
}

fn render_file_body(frame: &mut Frame<'_>, area: Rect, content: &str, scroll: u16) {
    let lines = if content.is_empty() {
        vec![Line::from("")]
    } else {
        content
            .lines()
            .map(|line| Line::from(line.to_string()))
            .collect()
    };
    frame.render_widget(
        Paragraph::new(Text::from(lines))
            .wrap(Wrap { trim: false })
            .scroll((scroll, 0)),
        area,
    );
}

fn render_hook_detail(frame: &mut Frame<'_>, area: Rect, hook: &files::HookEntry, scroll: u16) {
    let (_, command) = files::unwrap_shell(&hook.command);
    let lines = vec![
        Line::from(vec![
            Span::styled("event: ", Style::default().fg(Color::Magenta)),
            Span::raw(hook.event.clone()),
        ]),
        Line::from(vec![
            Span::styled("matcher: ", Style::default().fg(Color::Magenta)),
            Span::raw(value_or_any(&hook.matcher).to_string()),
        ]),
        Line::from(vec![
            Span::styled("source: ", Style::default().fg(Color::Magenta)),
            Span::raw(hook.source.clone()),
        ]),
        Line::from(""),
        Line::from(command),
    ];
    frame.render_widget(
        Paragraph::new(Text::from(lines))
            .wrap(Wrap { trim: false })
            .scroll((scroll, 0)),
        area,
    );
}

fn render_help(frame: &mut Frame<'_>, area: Rect) {
    frame.render_widget(Clear, area);
    let block = Block::default()
        .title(" Help ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
        .split(inner);
    for (column_index, column) in columns.iter().enumerate() {
        let groups = KEYBINDINGS
            .iter()
            .enumerate()
            .filter(|(index, _)| index % 2 == column_index)
            .map(|(_, group)| *group);
        let mut lines = Vec::new();
        for (group, bindings) in groups {
            lines.push(Line::from(Span::styled(
                group,
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            )));
            for (key, description) in bindings {
                lines.push(Line::from(vec![
                    Span::styled(
                        format!("{key:<12}"),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(*description, Style::default().fg(Color::DarkGray)),
                ]));
            }
            lines.push(Line::from(""));
        }
        frame.render_widget(Paragraph::new(Text::from(lines)), *column);
    }
}

#[must_use]
pub fn load_provider_targets(root: &Path) -> Vec<(String, String)> {
    let merged = config::load_merged_config(root).unwrap_or_default();
    let Ok(providers) = config::load_providers(&merged) else {
        return Vec::new();
    };
    let mut targets: Vec<(String, String)> = providers
        .into_iter()
        .map(|(name, config)| (name, config.target))
        .collect();
    targets.sort_by(|a, b| a.0.cmp(&b.0));
    targets
}

fn empty_dashboard_view() -> DashboardView {
    DashboardView {
        modules: Vec::new(),
        summary: StatusSummary::default(),
        provenance: Vec::new(),
        adrs: Vec::new(),
    }
}

fn column_block(title: &str, focused: bool) -> Block<'_> {
    Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(if focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        })
}

fn default_column_widths() -> MillerColumnWidths {
    MillerColumnWidths {
        left: LEFT_MIN_WIDTH,
        middle: MIDDLE_MIN_WIDTH,
    }
}

fn column_widths_for_rows(rows: &[ListRow]) -> MillerColumnWidths {
    let section_label_width = Section::ALL
        .iter()
        .map(|section| section.label().chars().count())
        .max()
        .unwrap_or_default();
    let left =
        usize_to_u16(section_label_width.saturating_add(6)).clamp(LEFT_MIN_WIDTH, LEFT_MAX_WIDTH);

    // Detail text renders only on the selected row, so the column sizes to
    // the labels; the selected row's detail may clip at the column edge and
    // is always fully visible in the detail pane.
    let row_width = rows
        .iter()
        .map(|row| row.label.chars().count())
        .max()
        .unwrap_or_default();
    let middle =
        usize_to_u16(row_width.saturating_add(8)).clamp(MIDDLE_MIN_WIDTH, MIDDLE_MAX_WIDTH);

    MillerColumnWidths { left, middle }
}

fn fit_miller_widths(total_width: u16, desired: MillerColumnWidths) -> MillerColumnWidths {
    let mut left = desired.left;
    let mut middle = desired.middle;
    let required = left.saturating_add(middle).saturating_add(MIN_DETAIL_WIDTH);
    if required <= total_width {
        return MillerColumnWidths { left, middle };
    }

    let mut overflow = required.saturating_sub(total_width);
    let middle_cut = middle.min(overflow);
    middle = middle.saturating_sub(middle_cut);
    overflow = overflow.saturating_sub(middle_cut);
    let left_cut = left.min(overflow);
    left = left.saturating_sub(left_cut);

    MillerColumnWidths { left, middle }
}

fn usize_to_u16(value: usize) -> u16 {
    u16::try_from(value).unwrap_or(u16::MAX)
}

fn selected_style(focused: bool) -> Style {
    if focused {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD)
    }
}

fn artifact_row(artifact: &ArtifactView, module: &str) -> ListRow {
    let warning = if artifact.has_broken_refs() || !artifact.sidecar_warning.is_empty() {
        " ⚠"
    } else {
        ""
    };
    ListRow::item(
        format!("{}{}", artifact.name, warning),
        module.to_string(),
        ListTarget::Artifact {
            module: module.to_string(),
            kind: artifact.kind.clone(),
            name: artifact.name.clone(),
        },
        artifact.overall_status(),
    )
}

/// gitui-style status letters: shape carries the state, color reinforces it.
fn status_dot(status: &str) -> &'static str {
    match status {
        "modified" => "M",
        "stale" => "!",
        "new" => "?",
        _ => "·",
    }
}

fn status_style(status: &str) -> Style {
    match status {
        "modified" => Style::default().fg(Color::Yellow),
        "stale" => Style::default().fg(Color::Red),
        "new" => Style::default().fg(Color::Magenta),
        _ => Style::default().fg(Color::DarkGray),
    }
}

fn value_or_any(value: &str) -> &str {
    if value.is_empty() { "any" } else { value }
}

/// Row index inside a bordered block for a click at (x, y), `None` when the
/// click lands on the border itself — borders focus a pane but never select.
fn bordered_row_at(region: Rect, x: u16, y: u16) -> Option<usize> {
    let inside_x = x > region.x && x.saturating_add(1) < region.x.saturating_add(region.width);
    let inside_y = y > region.y && y.saturating_add(1) < region.y.saturating_add(region.height);
    (inside_x && inside_y).then(|| usize::from(y - region.y - 1))
}

/// Maps a column inside the tab bar to its tab, mirroring the span layout in
/// `render_tabs`: one space then the label per tab. The space before a label
/// snaps to that tab so there are no dead cells between targets.
fn tab_at_column(column: u16) -> Option<DetailTab> {
    let mut cursor = 0u16;
    for tab in DetailTab::ALL {
        let width = u16::try_from(tab.label().chars().count()).unwrap_or(u16::MAX);
        let end = cursor.saturating_add(1).saturating_add(width);
        if column < end {
            return Some(tab);
        }
        cursor = end;
    }
    None
}

fn hint_row(focused: ColumnFocus) -> String {
    if focused == ColumnFocus::Detail {
        return [
            "Tab/p c d v f i n tabs",
            "1-9 sections",
            "j/k scroll",
            "m comment",
            "Y copy review",
            "? help",
        ]
        .join("  ·  ");
    }
    KEYBINDINGS
        .iter()
        .flat_map(|(_, bindings)| bindings.iter())
        .take(8)
        .map(|(key, description)| format!("{key} {description}"))
        .collect::<Vec<_>>()
        .join("  ·  ")
}

/// Greedy word-wrap for plain header text, needed because the preview
/// paragraph does not re-wrap glow output. A single word longer than the
/// width stays on its own line and clips.
fn wrap_plain(text: &str, width: usize) -> Vec<Line<'static>> {
    if width == 0 {
        return vec![Line::from(text.to_string())];
    }
    // Explicit newlines are paragraph structure; wrap each line separately.
    if text.contains('\n') {
        return text
            .lines()
            .flat_map(|line| wrap_plain(line, width))
            .collect();
    }
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        let candidate = current.chars().count() + 1 + word.chars().count();
        if !current.is_empty() && candidate > width {
            lines.push(Line::from(std::mem::take(&mut current)));
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(word);
    }
    if !current.is_empty() {
        lines.push(Line::from(current));
    }
    lines
}

/// One line of version-control truth for the artifact: branch (with
/// ahead/behind arrows), worktree state, last commit, and jj change id.
fn vcs_line(artifact: &ArtifactView) -> Option<Line<'static>> {
    use std::fmt::Write as _;
    let vcs = artifact.vcs.as_ref()?;
    let mut branch = vcs.branch.clone();
    if vcs.ahead > 0 {
        let _ = write!(branch, " ↑{}", vcs.ahead);
    }
    if vcs.behind > 0 {
        let _ = write!(branch, " ↓{}", vcs.behind);
    }
    let mut spans = vec![
        Span::styled(branch, Style::default().fg(Color::Cyan)),
        Span::raw(" · "),
    ];
    let (worktree_label, worktree_style) = match vcs.worktree {
        WorktreeState::Clean => ("✓ committed", Style::default().fg(Color::Green)),
        WorktreeState::Modified => ("⚠ uncommitted changes", Style::default().fg(Color::Yellow)),
        WorktreeState::Untracked => ("● untracked", Style::default().fg(Color::Magenta)),
    };
    spans.push(Span::styled(worktree_label, worktree_style));
    if let Some(commit) = artifact.git_log.first() {
        let sha_short: String = commit.sha.chars().take(7).collect();
        let date: String = commit.date.chars().take(10).collect();
        spans.push(Span::styled(
            format!(" · {sha_short} {date}"),
            Style::default().fg(Color::DarkGray),
        ));
        if !commit.jj_change.is_empty() {
            spans.push(Span::styled(
                format!(" · jj {}", commit.jj_change),
                Style::default().fg(Color::DarkGray),
            ));
        }
    } else if vcs.jj_colocated {
        spans.push(Span::styled(" · jj", Style::default().fg(Color::DarkGray)));
    }
    Some(Line::from(spans))
}

fn preview_lines_for_width(artifact: &ArtifactView, width: u16) -> (Vec<Line<'static>>, bool) {
    let mut lines = vec![
        Line::from(Span::styled(
            format!(
                "{} · {} · {}",
                artifact.kind, artifact.name, artifact.module
            ),
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(format!(
            "{} lines · {} · {} {}",
            artifact.total_lines(),
            value_or_any(&artifact.age_label()),
            if artifact.staleness_rank() == 0 {
                "✓"
            } else {
                "⚠"
            },
            artifact.staleness_label()
        )),
    ];
    if let Some(vcs) = vcs_line(artifact) {
        lines.push(vcs);
    }
    if !artifact.broken_refs.is_empty() {
        lines.extend(wrap_plain(
            &format!("broken refs: {}", artifact.broken_refs.join(", ")),
            width as usize,
        ));
    }
    if !artifact.description.is_empty() {
        lines.extend(wrap_plain(&artifact.description, width as usize));
    }
    // A rule separates the file's properties from its content.
    lines.push(Line::from(Span::styled(
        "─".repeat(usize::from(width)),
        Style::default().fg(Color::DarkGray),
    )));
    lines.push(Line::from(""));
    let body = if artifact.content_body.is_empty() {
        artifact.content_preview.as_str()
    } else {
        artifact.content_body.as_str()
    };
    if let Some(glow_lines) = rich::render_markdown_with_glow(body, width) {
        lines.extend(glow_lines);
        return (lines, true);
    }
    lines.extend(body.lines().map(|line| Line::from(line.to_string())));
    (lines, false)
}

/// The module name disambiguates artifacts that share a relative path across
/// modules (every module has a `skills/...` tree).
fn detail_cache_key(tab: DetailTab, module: &str, path: &str) -> String {
    format!("{tab:?}:{module}:{path}")
}

/// Whether unprocessed terminal input is queued. Errors (no terminal, as in
/// tests and snapshot mode) count as no pending input.
fn input_pending() -> bool {
    crossterm::event::poll(std::time::Duration::ZERO).unwrap_or(false)
}

/// Uncommitted changes to the artifact's source file, colored like a pager,
/// with a separator rule before each hunk header.
fn diff_lines(
    module: Option<&ModuleView>,
    artifact: &ArtifactView,
    width: u16,
) -> Vec<Line<'static>> {
    let header = Line::from(Span::styled(
        "Diff · uncommitted source changes",
        Style::default().add_modifier(Modifier::BOLD),
    ));
    let Some(repo) = module.and_then(|module| module.local_path.as_ref()) else {
        return vec![header, Line::from("no local repo for this module")];
    };
    let path = if artifact.source_path.is_empty() {
        artifact.relative_path.as_str()
    } else {
        artifact.source_path.as_str()
    };
    if artifact
        .vcs
        .as_ref()
        .is_some_and(|vcs| vcs.worktree == WorktreeState::Untracked)
    {
        // A new file has no diff against HEAD; show the whole body as added
        // so the reviewer can still inspect it here.
        let mut lines = vec![
            header,
            Line::from(vec![
                Span::styled("● ", Style::default().fg(Color::Magenta)),
                Span::raw("untracked file — whole body is new"),
            ]),
            Line::default(),
        ];
        match std::fs::read_to_string(repo.join(path)) {
            Ok(body) => lines.extend(body.lines().map(|line| {
                Line::from(Span::styled(
                    format!("+{line}"),
                    Style::default().fg(Color::Green),
                ))
            })),
            Err(error) => lines.push(Line::from(format!("could not read {path}: {error}"))),
        }
        return lines;
    }
    let output = std::process::Command::new("git")
        .args(["diff", "HEAD", "--", path])
        .current_dir(repo)
        .output();
    let Ok(output) = output else {
        return vec![header, Line::from("git diff failed to run")];
    };
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return vec![
            header,
            Line::from(format!("git diff failed: {}", stderr.trim())),
        ];
    }
    let diff = String::from_utf8_lossy(&output.stdout);
    if diff.trim().is_empty() {
        return vec![
            header,
            Line::from(vec![
                Span::styled("✓ ", Style::default().fg(Color::Green)),
                Span::raw("source file matches HEAD — no uncommitted changes"),
            ]),
        ];
    }
    let separator = "─".repeat(usize::from(width.max(8)).saturating_sub(2));
    let mut lines = vec![header, Line::default()];
    for raw in diff.lines() {
        if raw.starts_with("@@") {
            lines.push(Line::from(Span::styled(
                separator.clone(),
                Style::default().fg(Color::DarkGray),
            )));
        }
        lines.push(diff_line_colored(raw));
    }
    lines
}

/// The Deployments block of the provenance view: per-target verification
/// badges with per-harness rows.
fn deployment_lines(groups: &[commands::view::DeployGroup]) -> Vec<Line<'static>> {
    let mut lines = vec![Line::from(Span::styled(
        "Deployments",
        Style::default().add_modifier(Modifier::BOLD),
    ))];
    if groups.is_empty() {
        lines.push(Line::from(Span::styled(
            "not deployed anywhere",
            Style::default().fg(Color::DarkGray),
        )));
    }
    for group in groups {
        let all_verified = group.verified == group.total;
        lines.push(Line::from(vec![
            Span::styled(
                if all_verified { "✓ " } else { "✗ " },
                Style::default().fg(if all_verified {
                    Color::Green
                } else {
                    Color::Red
                }),
            ),
            Span::styled(
                group.target.clone(),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  {}/{} verified", group.verified, group.total),
                Style::default().fg(Color::DarkGray),
            ),
        ]));
        for harness in &group.harnesses {
            let (badge, style) = if harness.verified {
                ("✓", Style::default().fg(Color::Green))
            } else {
                ("✗ DRIFT", Style::default().fg(Color::Red))
            };
            lines.push(Line::from(vec![
                Span::raw("    "),
                Span::styled(
                    format!("{:<12}", harness.harness),
                    Style::default().fg(Color::Cyan),
                ),
                Span::styled(format!("{badge:<8}"), style),
                Span::styled(
                    harness.deployed_path.clone(),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        }
    }
    lines
}

/// Row offsets of hunk headers within rendered diff lines.
fn hunk_offsets(lines: &[Line<'_>]) -> Vec<usize> {
    lines
        .iter()
        .enumerate()
        .filter(|(_, line)| {
            line.spans
                .first()
                .is_some_and(|span| span.content.starts_with("@@"))
        })
        .map(|(index, _)| index)
        .collect()
}

fn diff_line_colored(line: &str) -> Line<'static> {
    let style = if line.starts_with("+++") || line.starts_with("---") {
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::BOLD)
    } else if line.starts_with('+') {
        Style::default().fg(Color::Green)
    } else if line.starts_with('-') {
        Style::default().fg(Color::Red)
    } else if line.starts_with("@@") {
        Style::default().fg(Color::Cyan)
    } else if line.starts_with("diff ") || line.starts_with("index ") {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default()
    };
    Line::from(Span::styled(line.to_string(), style))
}

fn frontmatter_lines(artifact: &ArtifactView, width: u16) -> Vec<Line<'static>> {
    if artifact.metadata.is_empty() {
        return vec![Line::from("no frontmatter metadata")];
    }
    let lines = artifact
        .metadata
        .iter()
        .map(|(key, value)| {
            Line::from(vec![
                Span::styled(format!("{key:<18}"), Style::default().fg(Color::Magenta)),
                Span::raw(value.clone()),
            ])
        })
        .collect();
    // Values wrap within the value column, never back to column zero.
    expand_gutter_wrapped(lines, 18, usize::from(width))
}

fn history_lines(artifact: &ArtifactView) -> Vec<Line<'static>> {
    if artifact.git_log.is_empty() {
        return vec![Line::from("no git history")];
    }
    let mut lines = Vec::new();
    for commit in &artifact.git_log {
        lines.push(Line::from(Span::styled(
            commit.message.clone(),
            Style::default().add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(format!(
            "{} · {} · {}",
            commit.date, commit.author, commit.sha
        )));
        if !commit.jj_change.is_empty() {
            lines.push(Line::from(format!("jj: {}", commit.jj_change)));
        }
        if !commit.checkpoint.is_empty() {
            lines.push(Line::from(format!(
                "checkpoint {} · {} sessions",
                commit.checkpoint, commit.session_count
            )));
            if !commit.prompt.is_empty() {
                lines.push(Line::from(format!("intent: {}", commit.prompt)));
            }
        }
        lines.push(Line::from(""));
    }
    lines
}

fn companion_lines(artifact: &ArtifactView) -> Vec<Line<'static>> {
    if artifact.companions.is_empty() {
        return vec![Line::from("no companions")];
    }
    let mut lines = Vec::new();
    for companion in &artifact.companions {
        lines.push(Line::from(Span::styled(
            companion.name.clone(),
            Style::default().add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(format!("path: {}", companion.relative_path)));
        if !companion.description.is_empty() {
            lines.push(Line::from(companion.description.clone()));
        }
        lines.push(Line::from(""));
        lines.extend(
            companion
                .content_body
                .lines()
                .map(|line| Line::from(line.to_string())),
        );
        lines.push(Line::from(""));
    }
    lines
}

fn copy_to_pbcopy(text: &str) -> bool {
    let Ok(mut child) = Command::new("pbcopy")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        return false;
    };
    let Some(mut stdin) = child.stdin.take() else {
        return false;
    };
    if stdin.write_all(text.as_bytes()).is_err() {
        return false;
    }
    drop(stdin);
    child.wait().is_ok_and(|status| status.success())
}

fn canonical_source(source: &str) -> String {
    source.trim_end_matches(".git").to_string()
}
