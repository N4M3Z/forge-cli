use std::path::PathBuf;

use ratatui::{Terminal, backend::TestBackend};

use commands::view::{ArtifactView, DashboardView, ModuleView, StatusSummary};

use super::{
    app::App,
    components::palette::{Palette, PaletteCommand},
};

fn fixture_view() -> DashboardView {
    let artifact = ArtifactView {
        name: "BuildSkill".to_string(),
        kind: "skills".to_string(),
        module: "forge-core".to_string(),
        relative_path: "skills/BuildSkill/SKILL.md".to_string(),
        description: "Build forge skills".to_string(),
        content_preview: "preview".to_string(),
        content_body: "full body".to_string(),
        ..ArtifactView::default()
    };

    DashboardView {
        modules: vec![ModuleView {
            name: "forge-core".to_string(),
            version: "0.1.0".to_string(),
            description: "core module".to_string(),
            source_uri: "https://github.com/N4M3Z/forge-core".to_string(),
            is_target: false,
            artifacts: vec![artifact],
        }],
        summary: StatusSummary::default(),
        provenance: Vec::new(),
        adrs: Vec::new(),
    }
}

fn fixture_app() -> App {
    App::from_view(PathBuf::from("."), Vec::new(), Vec::new(), fixture_view())
}

#[test]
fn app_constructs_from_fixture_snapshot() {
    let app = fixture_app();
    assert!(!app.should_quit());
}

#[test]
fn artifacts_pane_renders_artifact_name() {
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).expect("test backend");
    let mut app = fixture_app();

    terminal.draw(|frame| app.render(frame)).expect("render");

    let rendered: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect();
    assert!(rendered.contains("BuildSkill"));
}

#[test]
fn palette_parses_refresh() {
    assert_eq!(Palette::parse_command("refresh"), PaletteCommand::Refresh);
    assert_eq!(Palette::parse_command(" r "), PaletteCommand::Refresh);
}

#[test]
fn unknown_palette_command_sets_error() {
    let mut app = fixture_app();
    app.execute_palette_command(PaletteCommand::Unknown("wat".to_string()))
        .expect("unknown command is nonfatal");
    assert_eq!(app.palette_error.as_deref(), Some("unknown command: wat"));
}

#[test]
fn enter_opens_full_preview_rendering_the_body() {
    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).expect("test backend");
    let mut app = fixture_app();
    assert!(!app.is_preview_open());

    app.open_preview();
    assert!(app.is_preview_open());

    terminal.draw(|frame| app.render(frame)).expect("render");
    let rendered: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect();
    assert!(rendered.contains("full body"));
    assert!(rendered.contains("SKILL.md"));

    app.close_preview();
    assert!(!app.is_preview_open());
}
