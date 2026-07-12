pub mod app;
pub mod components;
pub mod event;
mod rich;
mod word_wrap;

use std::{
    io::{self, Stdout},
    path::PathBuf,
    time::Duration,
};

use crossterm::{
    cursor::{Hide, Show},
    event as terminal_event,
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend, backend::TestBackend};

use app::{App, DetailTab};

#[cfg(test)]
mod tests;

type TuiTerminal = Terminal<CrosstermBackend<Stdout>>;

pub fn run() -> i32 {
    match launch() {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("fatal: {error}");
            2
        }
    }
}

/// Render a single frame to plain text on stdout, for headless inspection of the
/// layout at a given size and view. Waits for the background scan to deliver real
/// data before drawing. This is the verification tool: run it, read the output.
pub fn run_snapshot(
    width: u16,
    height: u16,
    section: Option<usize>,
    tab: Option<&str>,
    drill: u8,
    row: usize,
) -> i32 {
    let mut app = App::load(PathBuf::from("."));
    for _ in 0..3000 {
        app.poll_scan();
        if !app.scan_pending() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    if let Some(number) = section {
        app.set_section_by_number(number);
    }
    for step in 0..drill {
        app.drill_or_expand();
        if step == 0 {
            for _ in 0..row {
                app.move_list_selection(1);
            }
        }
    }
    if let Some(detail_tab) = tab.and_then(detail_tab_from_name) {
        app.set_detail_tab(detail_tab);
    }
    let backend = TestBackend::new(width, height);
    let mut terminal = match Terminal::new(backend) {
        Ok(terminal) => terminal,
        Err(error) => {
            eprintln!("fatal: {error}");
            return 2;
        }
    };
    if let Err(error) = terminal.draw(|frame| app.render(frame)) {
        eprintln!("fatal: {error}");
        return 2;
    }
    let buffer = terminal.backend().buffer();
    for y in 0..buffer.area.height {
        let mut line = String::new();
        for x in 0..buffer.area.width {
            line.push_str(buffer[(x, y)].symbol());
        }
        println!("{}", line.trim_end());
    }
    0
}

fn detail_tab_from_name(name: &str) -> Option<DetailTab> {
    match name.to_ascii_lowercase().as_str() {
        "preview" => Some(DetailTab::Preview),
        "code" => Some(DetailTab::Code),
        "diff" => Some(DetailTab::Diff),
        "provenance" => Some(DetailTab::Provenance),
        "frontmatter" => Some(DetailTab::Frontmatter),
        "history" => Some(DetailTab::History),
        "companions" => Some(DetailTab::Companions),
        _ => None,
    }
}

fn launch() -> Result<(), Box<dyn std::error::Error>> {
    let mut app = App::load(PathBuf::from("."));
    let mut terminal = setup_terminal()?;
    install_panic_hook();

    let result = event_loop(&mut terminal, &mut app);
    restore_terminal(&mut terminal);
    result
}

fn setup_terminal() -> io::Result<TuiTerminal> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    if let Err(error) = execute!(stdout, EnterAlternateScreen, EnableMouseCapture, Hide) {
        let _ = disable_raw_mode();
        return Err(error);
    }
    match Terminal::new(CrosstermBackend::new(stdout)) {
        Ok(terminal) => Ok(terminal),
        Err(error) => {
            restore_terminal_without_backend();
            Err(error)
        }
    }
}

fn restore_terminal(terminal: &mut TuiTerminal) {
    let _ = disable_raw_mode();
    let _ = execute!(
        terminal.backend_mut(),
        DisableMouseCapture,
        LeaveAlternateScreen,
        Show
    );
    let _ = terminal.show_cursor();
}

fn restore_terminal_without_backend() {
    let _ = disable_raw_mode();
    let _ = execute!(
        io::stdout(),
        DisableMouseCapture,
        LeaveAlternateScreen,
        Show
    );
}

fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        restore_terminal_without_backend();
        default_hook(panic_info);
    }));
}

fn event_loop(terminal: &mut TuiTerminal, app: &mut App) -> Result<(), Box<dyn std::error::Error>> {
    while !app.should_quit() {
        app.poll_scan();
        terminal.draw(|frame| app.render(frame))?;
        if terminal_event::poll(Duration::from_millis(200))? {
            match terminal_event::read()? {
                terminal_event::Event::Key(key) => event::handle_key(app, key),
                terminal_event::Event::Mouse(mouse) => event::handle_mouse(app, mouse),
                _ => {}
            }
        }
        if let Some((program, directory)) = app.take_external() {
            run_external_tool(terminal, app, &program, &directory)?;
        }
    }
    Ok(())
}

/// Suspends the TUI, runs an external terminal tool (gitui/jjui) in the given
/// directory with the real terminal, and resumes when it exits.
fn run_external_tool(
    terminal: &mut TuiTerminal,
    app: &mut App,
    program: &str,
    directory: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    restore_terminal(terminal);
    let status = std::process::Command::new(program)
        .current_dir(directory)
        .status();
    *terminal = setup_terminal()?;
    terminal.clear()?;
    // The tool may have committed, amended, or touched files: rescan so VCS
    // state, diffs, and history reflect what it left behind. Force it — a
    // scan already in flight predates whatever the tool changed.
    app.force_refresh();
    match status {
        Ok(status) if status.success() => {}
        Ok(status) => app.set_toast(format!("{program} exited with {status}")),
        Err(error) => app.set_toast(format!("could not launch {program}: {error}")),
    }
    Ok(())
}
