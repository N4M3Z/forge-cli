pub mod app;
pub mod components;
pub mod event;

use std::{
    io::{self, Stdout},
    path::PathBuf,
    time::Duration,
};

use crossterm::{
    cursor::{Hide, Show},
    event as terminal_event, execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

use app::App;

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

fn launch() -> Result<(), Box<dyn std::error::Error>> {
    let mut app = App::load(PathBuf::from("."))?;
    let mut terminal = setup_terminal()?;
    install_panic_hook();

    let result = event_loop(&mut terminal, &mut app);
    restore_terminal(&mut terminal);
    result
}

fn setup_terminal() -> io::Result<TuiTerminal> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, Hide)?;
    let backend = CrosstermBackend::new(stdout);
    Terminal::new(backend)
}

fn restore_terminal(terminal: &mut TuiTerminal) {
    let _ = disable_raw_mode();
    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen, Show);
    let _ = terminal.show_cursor();
}

fn restore_terminal_without_backend() {
    let _ = disable_raw_mode();
    let _ = execute!(io::stdout(), LeaveAlternateScreen, Show);
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
        terminal.draw(|frame| app.render(frame))?;
        if terminal_event::poll(Duration::from_millis(200))?
            && let terminal_event::Event::Key(key) = terminal_event::read()?
        {
            event::handle_key(app, key)?;
        }
    }
    Ok(())
}
