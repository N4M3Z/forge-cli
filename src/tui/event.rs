use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};

use super::app::App;

pub fn handle_mouse(app: &mut App, mouse: MouseEvent) {
    if app.modal_blocks_mouse() {
        return;
    }
    match mouse.kind {
        MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
            app.clear_toast();
            app.mouse_click(mouse.column, mouse.row);
        }
        MouseEventKind::ScrollDown => app.mouse_scroll(mouse.column, mouse.row, true),
        MouseEventKind::ScrollUp => app.mouse_scroll(mouse.column, mouse.row, false),
        _ => {}
    }
}

pub fn handle_key(app: &mut App, key: KeyEvent) {
    app.clear_toast();
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
    if app.is_deploy_picker_open() {
        app.deploy_picker_key(key);
        return;
    }
    if app.is_launch_picker_open() {
        app.launch_picker_key(key);
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
    // Digits always address the numbered detail tabs; sections are reached
    // by navigation, the palette, and the letter shortcuts (t/h/c/m) shown
    // in the Sections column.
    if let KeyCode::Char(digit @ '1'..='6') = key.code {
        let index = usize::from(digit as u8 - b'1');
        app.set_detail_tab(super::app::DetailTab::ALL[index]);
        return;
    }
    if app.has_section_digit_shortcuts()
        && let KeyCode::Char(character) = key.code
        && app.set_section_by_shortcut(character)
    {
        return;
    }

    if !matches!(key.code, KeyCode::Char('q') | KeyCode::Esc) {
        app.disarm_quit();
    }
    match key.code {
        KeyCode::Esc => app.escape(),
        KeyCode::Char('q') => app.request_quit(),
        KeyCode::Char('?') | KeyCode::F(1) => app.toggle_help(),
        KeyCode::Char(':') => app.open_palette(),
        KeyCode::Char('/') => {
            app.set_section_by_number(9);
            app.begin_search_input();
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
        KeyCode::Char('v') => app.set_detail_tab(super::app::DetailTab::Provenance),
        KeyCode::Char('f') => app.set_detail_tab(super::app::DetailTab::Frontmatter),
        KeyCode::Char('i') => app.set_detail_tab(super::app::DetailTab::History),
        KeyCode::Char('o') => app.open_repo_tool(false),
        KeyCode::Char('O') => app.open_repo_tool(true),
        KeyCode::Char('D') => app.open_deploy_picker(),
        KeyCode::Char('L') => app.launch_harness(),
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
        KeyCode::Char(digit @ '1'..='6') => {
            let index = usize::from(digit as u8 - b'1');
            app.set_detail_tab(super::app::DetailTab::ALL[index]);
        }
        KeyCode::Char('p') => app.set_detail_tab(super::app::DetailTab::Preview),
        KeyCode::Char('c') => app.set_detail_tab(super::app::DetailTab::Code),
        KeyCode::Char('d') => app.set_detail_tab(super::app::DetailTab::Diff),
        KeyCode::Char('v') => app.set_detail_tab(super::app::DetailTab::Provenance),
        KeyCode::Char('f') => app.set_detail_tab(super::app::DetailTab::Frontmatter),
        KeyCode::Char('i') => app.set_detail_tab(super::app::DetailTab::History),
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
