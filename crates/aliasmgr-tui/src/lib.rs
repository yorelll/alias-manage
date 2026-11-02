//! Interactive terminal front end for Alias Manager.
//!
//! The TUI is a thin presentation layer: aliases, validation, sync, doctor,
//! loaders, and the one SQLite database all come from `aliasmgr-core`, exactly
//! like the CLI and GUI. The generated shell files and the current shell
//! session are never modified behind the user's back.

mod app;
mod data;
mod input;
mod model;
mod terminal;
mod text;
mod theme;
mod ui;

pub use app::App;
pub use theme::Theme;

use aliasmgr_core::{config::AppPaths, error::AliasError};
use std::{io::IsTerminal, path::PathBuf};

/// Runs the interactive UI against one config root.
///
/// Returns an error before touching the terminal when stdin/stdout are not
/// interactive, so `aliasmgr tui` in a pipeline or CI job fails clearly instead
/// of hanging. The alternate screen and raw mode are restored on every exit
/// path, including a panic.
pub fn run(config_dir: Option<PathBuf>) -> Result<(), AliasError> {
    let root = AppPaths::discover(config_dir.as_deref()).root;
    terminal::check_interactive(
        std::io::stdin().is_terminal(),
        std::io::stdout().is_terminal(),
    )?;
    let mut app = App::new(root)?;
    terminal::install_panic_hook();
    let mut session = match terminal::TerminalSession::enter() {
        Ok(session) => session,
        Err(error) => {
            terminal::restore_panic_hook();
            return Err(AliasError::Io(error));
        }
    };
    let result = event_loop(session.terminal_mut(), &mut app);
    drop(session);
    terminal::restore_panic_hook();
    result
}

fn event_loop(
    terminal: &mut ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>,
    app: &mut App,
) -> Result<(), AliasError> {
    use crossterm::event::{self, Event};
    use std::time::Duration;

    loop {
        if app.dirty {
            terminal.draw(|frame| ui::render(frame, app))?;
            app.dirty = false;
        }
        if app.should_quit {
            break;
        }
        // 250 ms keeps background jobs and external-change polling responsive
        // without a busy loop; no event means no redraw.
        if event::poll(Duration::from_millis(250))? {
            match event::read()? {
                Event::Key(key) => app.handle_key(key),
                Event::Resize(_, _) => app.on_resize(),
                _ => {}
            }
        }
        app.poll_jobs();
        app.poll_external_changes();
    }
    Ok(())
}
