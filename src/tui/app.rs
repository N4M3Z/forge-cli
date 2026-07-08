use std::path::{Path, PathBuf};

use crossterm::event::KeyEvent;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
};

use commands::{error::Error, services, view::DashboardView};

use crate::cli::{config, watchlist};

use super::components::{
    Component,
    artifacts::ArtifactsPane,
    palette::{Palette, PaletteCommand},
    preview::ArtifactPreview,
    projects::ProjectsPane,
    provenance::ProvenancePane,
    sources::SourcesPane,
};

const PANE_COUNT: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pane {
    Artifacts = 0,
    Provenance = 1,
    Projects = 2,
    Sources = 3,
}

#[derive(Debug, Clone)]
pub struct App {
    root: PathBuf,
    providers: Vec<(String, String)>,
    watched_locations: Vec<PathBuf>,
    view: DashboardView,
    focused: Pane,
    should_quit: bool,
    pub palette_error: Option<String>,
    preview: Option<ArtifactPreview>,
    artifacts: ArtifactsPane,
    provenance: ProvenancePane,
    projects: ProjectsPane,
    sources: SourcesPane,
    palette: Palette,
}

impl App {
    pub fn load(root: PathBuf) -> Result<Self, Error> {
        let providers = load_provider_targets(&root);
        let watched_locations = watchlist::watched_locations();
        let view = services::build_view(&root, &providers, &watched_locations)?;
        Ok(Self::from_view(root, providers, watched_locations, view))
    }

    #[must_use]
    pub fn from_view(
        root: PathBuf,
        providers: Vec<(String, String)>,
        watched_locations: Vec<PathBuf>,
        view: DashboardView,
    ) -> Self {
        let artifacts = ArtifactsPane::new(view.clone());
        let provenance = ProvenancePane::new(view.clone());
        let projects = ProjectsPane::new(view.clone());
        let sources = SourcesPane::new(view.clone(), watched_locations.clone());
        let mut app = Self {
            root,
            providers,
            watched_locations,
            view,
            focused: Pane::Artifacts,
            should_quit: false,
            palette_error: None,
            preview: None,
            artifacts,
            provenance,
            projects,
            sources,
            palette: Palette::new(),
        };
        app.sync_components();
        app
    }

    pub fn refresh(&mut self) -> Result<(), Error> {
        self.view = services::build_view(&self.root, &self.providers, &self.watched_locations)?;
        self.sync_snapshot();
        self.sync_components();
        Ok(())
    }

    pub fn render(&mut self, frame: &mut Frame<'_>) {
        if let Some(preview) = self.preview.as_mut() {
            preview.render(frame, frame.area());
            return;
        }
        self.sync_components();
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(3)])
            .split(frame.area());

        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(layout[0]);
        let top = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(rows[0]);
        let bottom = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(rows[1]);

        self.artifacts.render(frame, top[0]);
        self.provenance.render(frame, top[1]);
        self.projects.render(frame, bottom[0]);
        self.sources.render(frame, bottom[1]);
        self.palette
            .render_with_error(frame, layout[1], self.palette_error.as_deref());
    }

    pub fn request_quit(&mut self) {
        self.should_quit = true;
    }

    #[must_use]
    pub fn is_preview_open(&self) -> bool {
        self.preview.is_some()
    }

    pub fn open_preview(&mut self) {
        if let Some(artifact) = self.artifacts.selected_artifact() {
            self.preview = Some(ArtifactPreview::from_artifact(artifact));
        }
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
    pub fn should_quit(&self) -> bool {
        self.should_quit
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

    pub fn execute_palette(&mut self) -> Result<(), Error> {
        let command = self.palette.take_command();
        self.execute_palette_command(command)
    }

    pub fn execute_palette_command(&mut self, command: PaletteCommand) -> Result<(), Error> {
        self.palette_error = None;
        match command {
            PaletteCommand::Refresh => self.refresh(),
            PaletteCommand::Quit => {
                self.request_quit();
                Ok(())
            }
            PaletteCommand::Find(query) => {
                self.sources.set_query(query);
                self.focused = Pane::Sources;
                Ok(())
            }
            PaletteCommand::Empty => Ok(()),
            PaletteCommand::Unknown(verb) => {
                self.palette_error = Some(format!("unknown command: {verb}"));
                Ok(())
            }
        }
    }

    pub fn focus_next(&mut self) {
        self.focused = pane_from_index((self.focused as usize + 1) % PANE_COUNT);
    }

    pub fn focus_previous(&mut self) {
        self.focused = pane_from_index((self.focused as usize + PANE_COUNT - 1) % PANE_COUNT);
    }

    pub fn focused_key(&mut self, key: KeyEvent) {
        match self.focused {
            Pane::Artifacts => {
                let _ = self.artifacts.on_key(key);
            }
            Pane::Provenance => {
                let _ = self.provenance.on_key(key);
            }
            Pane::Projects => {
                let _ = self.projects.on_key(key);
            }
            Pane::Sources => {
                let _ = self.sources.on_key(key);
            }
        }
    }

    fn sync_components(&mut self) {
        self.artifacts.set_focused(self.focused == Pane::Artifacts);
        self.provenance
            .set_focused(self.focused == Pane::Provenance);
        self.projects.set_focused(self.focused == Pane::Projects);
        self.sources.set_focused(self.focused == Pane::Sources);

        let selected = self.artifacts.selected_artifact().cloned();
        self.provenance.set_selected_artifact(selected);
    }

    fn sync_snapshot(&mut self) {
        self.artifacts.set_view(self.view.clone());
        self.provenance.set_view(self.view.clone());
        self.projects.set_view(self.view.clone());
        self.sources
            .set_view(self.view.clone(), self.watched_locations.clone());
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

fn pane_from_index(index: usize) -> Pane {
    match index {
        0 => Pane::Artifacts,
        1 => Pane::Provenance,
        2 => Pane::Projects,
        _ => Pane::Sources,
    }
}
