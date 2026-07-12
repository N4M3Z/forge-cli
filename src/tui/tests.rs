use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend};

use commands::{
    manifest::FileStatus,
    services::files::{
        ConfigFile, FileSections, HarnessFiles, HarnessHooks, HookEntry, SchemaGroup,
    },
    view::{
        Adr, ArtifactView, DashboardView, GitCommit, ModuleView, ProvenanceArtifact,
        ProvenanceView, ProviderStatus, StatusSummary,
    },
};

use super::{
    app::{App, ColumnFocus, CommentKind, DetailTab, KEYBINDINGS, Section},
    components::palette::{Palette, PaletteCommand},
    event,
};

fn buffer_position(output: &str, needle: &str) -> (u16, u16) {
    let byte_index = output.find(needle).expect("needle rendered");
    let cell_index = output[..byte_index].chars().count();
    (
        u16::try_from(cell_index % 120).expect("x fits"),
        u16::try_from(cell_index / 120).expect("y fits"),
    )
}

fn fixture_view() -> DashboardView {
    let mut providers = std::collections::BTreeMap::new();
    providers.insert(
        "claude".to_string(),
        ProviderStatus {
            status: FileStatus::Unchanged,
            fingerprint: Some("abc123".to_string()),
        },
    );

    let artifact = ArtifactView {
        name: "BuildSkill".to_string(),
        kind: "skills".to_string(),
        module: "forge-core".to_string(),
        relative_path: "skills/BuildSkill/SKILL.md".to_string(),
        description: "Build forge skills".to_string(),
        content_preview: "preview".to_string(),
        content_body: "full body".to_string(),
        raw_source: "---\ndescription: Build forge skills\n---\nfull body".to_string(),
        metadata: vec![("description".to_string(), "Build forge skills".to_string())],
        providers,
        git_log: vec![GitCommit {
            sha: "abcdef1".to_string(),
            message: "Implement skill".to_string(),
            date: "2026-01-02".to_string(),
            author: "N4M3Z".to_string(),
            checkpoint: "123456789abc".to_string(),
            prompt: "Make the skill useful".to_string(),
            session_count: 2,
            jj_change: "zzzzzzzz".to_string(),
        }],
        ..ArtifactView::default()
    };

    DashboardView {
        modules: vec![
            ModuleView {
                name: "forge-core".to_string(),
                version: "0.1.0".to_string(),
                description: "core module".to_string(),
                source_uri: "https://github.com/N4M3Z/forge-core".to_string(),
                is_target: false,
                artifacts: vec![artifact],
                local_path: None,
                vcs: None,
                git_log: Vec::new(),
            },
            ModuleView {
                name: "project-target".to_string(),
                version: "0.1.0".to_string(),
                description: "target module".to_string(),
                source_uri: "https://github.com/N4M3Z/project-target".to_string(),
                is_target: true,
                artifacts: Vec::new(),
                local_path: None,
                vcs: None,
                git_log: Vec::new(),
            },
        ],
        summary: StatusSummary {
            unchanged: 1,
            stale: 0,
            modified: 0,
            new: 0,
        },
        provenance: vec![ProvenanceView {
            source_uri: "https://github.com/N4M3Z/forge-core".to_string(),
            verified: 1,
            total: 1,
            orphans: Vec::new(),
            artifacts: vec![ProvenanceArtifact {
                deployed_path: "skills/BuildSkill/SKILL.md".to_string(),
                source_path: "skills/BuildSkill/SKILL.md".to_string(),
                harness: "claude".to_string(),
                target: "target-one".to_string(),
                verified: true,
                deployed_sha: "abc123".to_string(),
                expected_sha: "abc123".to_string(),
                input_sha: "abc123".to_string(),
            }],
        }],
        adrs: vec![Adr {
            id: "ADR-0001".to_string(),
            title: "Use Miller columns".to_string(),
            status: "accepted".to_string(),
            repo: "forge-core".to_string(),
            source_uri: "https://github.com/N4M3Z/forge-core".to_string(),
            relative_path: "docs/decisions/ADR-0001.md".to_string(),
            state: "authored".to_string(),
            source: String::new(),
            summary: "Context summary".to_string(),
            local_path: String::new(),
        }],
    }
}

fn fixture_app() -> App {
    App::from_view_with_files(
        PathBuf::from("."),
        Vec::new(),
        Vec::new(),
        fixture_view(),
        fixture_file_sections(),
    )
}

fn fixture_file_sections() -> FileSections {
    let settings_file = ConfigFile {
        label: "settings.json".to_string(),
        path: "~/.claude/settings.json".to_string(),
        language: "json".to_string(),
        content: r#"{"hooks":{"PreToolUse":[{"matcher":"Write","hooks":[{"command":"bash -c 'echo fixture-hook'"}]}]}}"#
            .to_string(),
    };
    FileSections {
        settings: vec![HarnessFiles {
            harness: "claude".to_string(),
            files: vec![settings_file.clone()],
        }],
        hooks: vec![HarnessHooks {
            harness: "claude".to_string(),
            hooks: vec![HookEntry {
                event: "PreToolUse".to_string(),
                matcher: "Write".to_string(),
                command: "bash -c 'echo fixture-hook'".to_string(),
                source: "~/.claude/settings.json".to_string(),
            }],
        }],
        config: vec![ConfigFile {
            label: "Module manifest".to_string(),
            path: "./module.yaml".to_string(),
            language: "yaml".to_string(),
            content: "name: forge-fixture\n".to_string(),
        }],
        schemas: vec![SchemaGroup {
            source: "forge-core".to_string(),
            files: vec![ConfigFile {
                label: "skills/.mdschema".to_string(),
                path: "./skills/.mdschema".to_string(),
                language: "yaml".to_string(),
                content: "kind: skills\n".to_string(),
            }],
        }],
    }
}

fn rendered(app: &mut App) -> String {
    let backend = TestBackend::new(120, 32);
    let mut terminal = Terminal::new(backend).expect("test backend");
    terminal.draw(|frame| app.render(frame)).expect("render");
    terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect()
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

#[test]
fn app_loads_in_background_and_renders_scanning_shell() {
    let temp = tempfile::tempdir().expect("tempdir");
    let mut app = App::load(temp.path().to_path_buf());

    let output = rendered(&mut app);

    assert!(output.contains("Scanning modules"));
    assert!(output.contains("Sections"));
    assert!(output.contains("Overview"));
}

#[test]
fn miller_sections_and_skills_list_render() {
    let mut app = fixture_app();
    app.set_section_by_number(2);

    let output = rendered(&mut app);

    assert!(output.contains("Overview"));
    assert!(output.contains("Skills"));
    assert!(output.contains("BuildSkill"));
}

#[test]
fn drilling_to_skill_detail_renders_body() {
    let mut app = fixture_app();
    app.set_section_by_number(2);
    app.focus_next();
    app.drill_or_expand();

    let output = rendered(&mut app);

    assert!(output.contains("full body"));
    assert!(output.contains("Build forge skills"));
}

#[test]
fn provenance_and_history_tabs_render_scanned_data() {
    let mut app = fixture_app();
    app.set_section_by_number(2);
    app.focus_next();
    app.drill_or_expand();
    app.set_detail_tab(DetailTab::Provenance);

    let provenance = rendered(&mut app);
    assert!(provenance.contains("target-one"));
    assert!(provenance.contains("OK"));

    app.set_detail_tab(DetailTab::History);
    let history = rendered(&mut app);
    assert!(history.contains("Implement skill"));
    assert!(history.contains("Make the skill useful"));
}

#[test]
fn adrs_section_lists_fixture_adr() {
    let mut app = fixture_app();
    app.set_section_by_number(6);

    let output = rendered(&mut app);

    assert!(output.contains("ADR-0001"));
    assert!(output.contains("Use Miller columns"));
}

#[test]
fn enter_on_repositories_does_not_open_artifact_preview() {
    let mut app = fixture_app();
    app.set_section_by_number(5);
    app.focus_next();

    event::handle_key(&mut app, key(KeyCode::Enter));

    assert!(!app.is_preview_open());
    let output = rendered(&mut app);
    assert!(output.contains("project-target"));
    assert!(!output.contains("full body"));
}

#[test]
fn help_overlay_renders_known_binding_and_quit() {
    let mut app = fixture_app();

    event::handle_key(&mut app, key(KeyCode::Char('?')));
    let output = rendered(&mut app);

    assert!(output.contains('?'));
    assert!(output.contains("quit"));
}

#[test]
fn keybindings_table_drives_help_and_hint_row() {
    let binding = KEYBINDINGS
        .iter()
        .flat_map(|(_, bindings)| bindings.iter())
        .find(|(key, _)| *key == "h/j/k/l")
        .expect("navigation binding");
    assert_eq!(binding.1, "move, drill, and go back");

    let mut app = fixture_app();
    let hint = rendered(&mut app);
    assert!(hint.contains("h/j/k/l move, drill, and go back"));

    event::handle_key(&mut app, key(KeyCode::Char('?')));
    let help = rendered(&mut app);
    assert!(help.contains("move, drill, and go back"));
}

#[test]
fn palette_parses_dashboard_parity_commands() {
    assert_eq!(Palette::parse_command("refresh"), PaletteCommand::Refresh);
    assert_eq!(Palette::parse_command(" r "), PaletteCommand::Refresh);
    assert_eq!(
        Palette::parse_command("find build"),
        PaletteCommand::Find("build".to_string())
    );
    assert_eq!(
        Palette::parse_command("skills"),
        PaletteCommand::GoTo("skills".to_string())
    );
    assert_eq!(
        Palette::parse_command("sort staleness"),
        PaletteCommand::Sort("staleness".to_string())
    );
    assert_eq!(
        Palette::parse_command("filter attention"),
        PaletteCommand::Filter("attention".to_string())
    );
    assert_eq!(
        Palette::parse_command("settings"),
        PaletteCommand::GoTo("settings".to_string())
    );
    assert_eq!(
        Palette::parse_command("hooks"),
        PaletteCommand::GoTo("hooks".to_string())
    );
    assert_eq!(
        Palette::parse_command("config"),
        PaletteCommand::GoTo("config".to_string())
    );
    assert_eq!(
        Palette::parse_command("schemas"),
        PaletteCommand::GoTo("schemas".to_string())
    );
}

#[test]
fn unknown_palette_command_sets_error() {
    let mut app = fixture_app();
    app.execute_palette_command(PaletteCommand::Unknown("wat".to_string()));
    assert_eq!(app.palette_error.as_deref(), Some("unknown command: wat"));
}

#[test]
fn search_input_mode_is_explicit() {
    let mut app = fixture_app();
    event::handle_key(&mut app, key(KeyCode::Char('/')));

    for character in ['h', 'e', 'l', 'l', 'o'] {
        event::handle_key(&mut app, key(KeyCode::Char(character)));
    }
    assert_eq!(app.search_query(), "hello");
    assert_eq!(app.section(), Section::Search);

    event::handle_key(&mut app, key(KeyCode::Enter));
    event::handle_key(&mut app, key(KeyCode::Char('j')));
    assert_eq!(app.search_query(), "hello");
}

#[test]
fn detail_digit_shortcut_selects_tab_without_changing_section() {
    let mut app = fixture_app();
    app.focus_next();
    app.focus_next();

    event::handle_key(&mut app, key(KeyCode::Char('2')));

    assert_eq!(app.detail_tab(), DetailTab::Code);
    assert_eq!(app.section(), Section::Overview);
}

#[test]
fn render_reuses_cached_list_rows_between_frames() {
    let mut app = fixture_app();

    let _ = rendered(&mut app);
    assert_eq!(app.row_build_count(), 1);

    let _ = rendered(&mut app);
    assert_eq!(app.row_build_count(), 1);
}

#[test]
fn miller_columns_give_detail_the_remaining_width() {
    let mut app = fixture_app();
    app.set_section_by_number(2);

    let widths = app.column_widths_for_total(120);
    let detail_width = 120_u16.saturating_sub(widths.left + widths.middle);

    assert!((14..=20).contains(&widths.left));
    assert!((24..=40).contains(&widths.middle));
    assert!(detail_width > widths.left);
    assert!(detail_width > widths.middle);
}

#[test]
fn miller_columns_shrink_fixed_columns_before_detail_on_narrow_widths() {
    let mut app = fixture_app();
    app.set_section_by_number(2);

    let widths = app.column_widths_for_total(50);
    let detail_width = 50_u16.saturating_sub(widths.left + widths.middle);

    assert!(detail_width >= 20);
}

#[test]
fn rich_detail_caches_are_reused_between_frames() {
    let mut app = fixture_app();
    app.set_section_by_number(2);
    app.focus_next();
    app.drill_or_expand();

    let _ = rendered(&mut app);
    assert_eq!(app.preview_cache_build_count(), 1);
    let _ = rendered(&mut app);
    assert_eq!(app.preview_cache_build_count(), 1);

    app.set_detail_tab(DetailTab::Code);
    let _ = rendered(&mut app);
    assert_eq!(app.code_cache_build_count(), 1);
    let _ = rendered(&mut app);
    assert_eq!(app.code_cache_build_count(), 1);
}

#[test]
fn tuicr_digest_exports_line_comments() {
    let mut app = fixture_app();
    app.add_comment_for_test(
        "forge-core",
        "skills/BuildSkill/SKILL.md",
        3,
        CommentKind::Issue,
        "tighten the wording",
    );

    let digest = app.tuicr_digest();

    assert!(digest.contains("**[ISSUE]** `skills/BuildSkill/SKILL.md:3`"));
}

#[test]
fn mouse_click_selects_section_and_focuses() {
    let mut app = fixture_app();
    let output = rendered(&mut app);
    let (x, y) = buffer_position(&output, "2 Skills");

    app.mouse_click(x, y);

    assert_eq!(app.section(), Section::Skills);
    assert_eq!(app.focused_column(), ColumnFocus::Sections);
}

#[test]
fn mouse_click_on_tab_switches_detail_tab() {
    let mut app = fixture_app();
    app.set_section_by_number(2);
    app.drill_or_expand();
    app.drill_or_expand();
    let output = rendered(&mut app);
    let (x, y) = buffer_position(&output, "3 Diff");

    app.mouse_click(x, y);

    assert_eq!(app.detail_tab(), DetailTab::Diff);
    assert_eq!(app.focused_column(), ColumnFocus::Detail);
}

#[test]
fn mouse_wheel_scrolls_detail_without_moving_selection() {
    let mut app = fixture_app();
    app.set_section_by_number(2);
    app.drill_or_expand();
    app.drill_or_expand();
    let output = rendered(&mut app);
    let (x, y) = buffer_position(&output, "1 Preview");
    let selected_before = app.selected_row_for_test();

    app.mouse_scroll(x, y + 2, true);

    assert_eq!(app.selected_row_for_test(), selected_before);
    assert_eq!(app.detail_scroll_for_test(), 3);
}

#[test]
fn comment_prompt_opens_from_preview_tab() {
    let mut app = fixture_app();
    app.set_section_by_number(2);
    app.drill_or_expand();
    app.drill_or_expand();
    assert_eq!(app.detail_tab(), DetailTab::Preview);

    event::handle_key(&mut app, key(KeyCode::Char('m')));

    assert!(app.is_comment_prompt_open());
    assert_eq!(app.detail_tab(), DetailTab::Code);
}

#[test]
fn tuicr_comment_kind_cycles_in_order() {
    assert_eq!(CommentKind::Issue.next(), CommentKind::Note);
    assert_eq!(CommentKind::Note.next(), CommentKind::Suggestion);
    assert_eq!(CommentKind::Suggestion.next(), CommentKind::Praise);
    assert_eq!(CommentKind::Praise.next(), CommentKind::Issue);
}

#[test]
fn settings_section_lists_fixture_file_and_detail_body() {
    let mut app = fixture_app();
    app.execute_palette_command(PaletteCommand::GoTo("settings".to_string()));

    let output = rendered(&mut app);
    assert!(output.contains("claude"));
    assert!(output.contains("settings.json"));

    app.focus_next();
    app.drill_or_expand();
    let detail = rendered(&mut app);
    assert!(detail.contains("PreToolUse"));
    assert!(detail.contains("fixture-hook"));
}

#[test]
fn hooks_section_lists_fixture_hook_and_detail() {
    let mut app = fixture_app();
    app.execute_palette_command(PaletteCommand::GoTo("hooks".to_string()));

    let output = rendered(&mut app);
    assert!(output.contains("PreToolUse"));
    assert!(output.contains("Write"));
    assert!(output.contains("echo fixture-hook"));

    app.focus_next();
    app.drill_or_expand();
    let detail = rendered(&mut app);
    assert!(detail.contains("source:"));
    assert!(detail.contains("~/.claude/settings.json"));
    assert!(detail.contains("echo fixture-hook"));
}
