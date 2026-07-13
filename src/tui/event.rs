use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::app::App;

pub fn handle_key(app: &mut App, key: KeyEvent) {
    if app.is_preview_open() {
        handle_preview_key(app, key);
        return;
    }
    if app.is_help_open() {
        match key.code {
            KeyCode::Esc | KeyCode::Char('?') | KeyCode::F(1) => app.close_help(),
            _ => {}
        }
        return;
    }
    if app.is_palette_open() {
        handle_palette_key(app, key);
        return;
    }
    if app.is_comment_prompt_open() {
        app.comment_prompt_key(key);
        return;
    }
    if app.is_search_input_active()
        && matches!(
            key.code,
            KeyCode::Char(_) | KeyCode::Backspace | KeyCode::Esc | KeyCode::Enter
        )
    {
        app.search_input_key(key);
        return;
    }
    if app.has_section_digit_shortcuts()
        && let KeyCode::Char(character) = key.code
        && app.set_section_by_shortcut(character)
    {
        return;
    }

    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => app.request_quit(),
        KeyCode::Char('?') | KeyCode::F(1) => app.toggle_help(),
        KeyCode::Char(':') => app.open_palette(),
        KeyCode::Char('/') => {
            app.set_section_by_number(9);
        }
        KeyCode::Char('r') => app.refresh(),
        KeyCode::Char('Y') => app.copy_tuicr_review(),
        KeyCode::Char('y') => app.copy_selected(),
        KeyCode::Enter | KeyCode::Right | KeyCode::Char('l') => app.drill_or_expand(),
        KeyCode::Left | KeyCode::Char('h') => app.move_back(),
        KeyCode::Tab if key.modifiers.contains(KeyModifiers::SHIFT) => app.focus_previous(),
        KeyCode::BackTab => app.focus_previous(),
        KeyCode::Tab => app.focus_next(),
        KeyCode::Char('p') => app.set_detail_tab(super::app::DetailTab::Preview),
        KeyCode::Char('c') => app.set_detail_tab(super::app::DetailTab::Code),
        KeyCode::Char('d') => app.set_detail_tab(super::app::DetailTab::Diff),
        _ => app.focused_key(key),
    }
}

fn handle_preview_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Enter => app.close_preview(),
        KeyCode::Down | KeyCode::Char('j') => app.preview_scroll_down(1),
        KeyCode::Up | KeyCode::Char('k') => app.preview_scroll_up(1),
        KeyCode::PageDown | KeyCode::Char(' ') => app.preview_scroll_down(10),
        KeyCode::PageUp | KeyCode::Char('b') => app.preview_scroll_up(10),
        KeyCode::Home | KeyCode::Char('g') => app.preview_scroll_to_top(),
        KeyCode::End | KeyCode::Char('G') => app.preview_scroll_to_bottom(),
        _ => {}
    }
}

fn handle_palette_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => app.close_palette(),
        KeyCode::Enter => app.execute_palette(),
        _ => app.palette_key(key),
    }
}
