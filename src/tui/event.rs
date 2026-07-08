use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use commands::error::Error;

use super::app::App;

pub fn handle_key(app: &mut App, key: KeyEvent) -> Result<(), Error> {
    if app.is_preview_open() {
        handle_preview_key(app, key);
        return Ok(());
    }
    if app.is_palette_open() {
        return handle_palette_key(app, key);
    }

    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => app.request_quit(),
        KeyCode::Char(':') => app.open_palette(),
        KeyCode::Char('r') => app.refresh()?,
        KeyCode::Enter => app.open_preview(),
        KeyCode::Tab if key.modifiers.contains(KeyModifiers::SHIFT) => app.focus_previous(),
        KeyCode::BackTab => app.focus_previous(),
        KeyCode::Tab => app.focus_next(),
        _ => app.focused_key(key),
    }
    Ok(())
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

fn handle_palette_key(app: &mut App, key: KeyEvent) -> Result<(), Error> {
    match key.code {
        KeyCode::Esc => app.close_palette(),
        KeyCode::Enter => app.execute_palette()?,
        _ => app.palette_key(key),
    }
    Ok(())
}
