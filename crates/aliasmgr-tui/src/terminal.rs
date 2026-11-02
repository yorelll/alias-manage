//! Terminal lifecycle: interactive check, raw mode/alternate screen, and a
//! panic hook that restores the terminal even if a render panics.

use aliasmgr_core::error::AliasError;
use crossterm::{
    cursor::Show,
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::{
    io::{self, Stdout, Write},
    panic::{self, PanicHookInfo},
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex, OnceLock,
    },
};

type Hook = Box<dyn Fn(&PanicHookInfo<'_>) + Sync + Send + 'static>;

static PREVIOUS_HOOK: OnceLock<Mutex<Option<Hook>>> = OnceLock::new();
static HOOK_INSTALLED: AtomicBool = AtomicBool::new(false);

fn previous_hook() -> &'static Mutex<Option<Hook>> {
    PREVIOUS_HOOK.get_or_init(|| Mutex::new(None))
}

/// A non-TTY invocation must fail clearly instead of blocking on crossterm or
/// printing escape sequences into a pipe.
pub fn check_interactive(stdin_tty: bool, stdout_tty: bool) -> Result<(), AliasError> {
    if stdin_tty && stdout_tty {
        Ok(())
    } else {
        Err(AliasError::Config(
            "the TUI requires an interactive terminal (stdin and stdout must both be TTYs); \
use the non-interactive CLI commands instead"
                .to_string(),
        ))
    }
}

/// Writes the escape sequences that leave the alternate screen and show the
/// cursor. Split out so tests can verify the restore sequence without a TTY.
pub fn write_restore_commands(mut output: impl Write) -> io::Result<()> {
    execute!(output, LeaveAlternateScreen, Show)
}

fn restore_terminal() {
    let _ = disable_raw_mode();
    let mut stdout = io::stdout();
    let _ = write_restore_commands(&mut stdout);
    let _ = stdout.flush();
}

/// Best-effort restore used by the panic hook and by `Drop`.
pub fn emergency_restore() {
    restore_terminal();
}

/// Chains a restore into the existing panic hook exactly once.
pub fn install_panic_hook() {
    if HOOK_INSTALLED.swap(true, Ordering::SeqCst) {
        return;
    }
    let previous = panic::take_hook();
    if let Ok(mut slot) = previous_hook().lock() {
        *slot = Some(previous);
    }
    panic::set_hook(Box::new(|info| {
        emergency_restore();
        if let Ok(slot) = previous_hook().lock() {
            if let Some(previous) = slot.as_ref() {
                previous(info);
            }
        }
    }));
}

/// Restores the hook that was active before [`install_panic_hook`].
pub fn restore_panic_hook() {
    if !HOOK_INSTALLED.swap(false, Ordering::SeqCst) {
        return;
    }
    let previous = previous_hook().lock().ok().and_then(|mut slot| slot.take());
    if let Some(previous) = previous {
        panic::set_hook(previous);
    }
}

/// Owns raw mode and the alternate screen. Dropping it restores both, so every
/// early return and every error path after entering the session is safe.
pub struct TerminalSession {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    restored: bool,
}

impl TerminalSession {
    pub fn enter() -> io::Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        if let Err(error) = execute!(stdout, EnterAlternateScreen) {
            let _ = disable_raw_mode();
            return Err(error);
        }
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = match Terminal::new(backend) {
            Ok(terminal) => terminal,
            Err(error) => {
                restore_terminal();
                return Err(error);
            }
        };
        if let Err(error) = terminal.clear() {
            restore_terminal();
            return Err(error);
        }
        Ok(Self {
            terminal,
            restored: false,
        })
    }

    pub fn terminal_mut(&mut self) -> &mut Terminal<CrosstermBackend<Stdout>> {
        &mut self.terminal
    }

    fn restore(&mut self) {
        if !self.restored {
            restore_terminal();
            self.restored = true;
        }
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        self.restore();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    static HOOK_TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn non_tty_is_rejected_with_an_actionable_error() {
        for (stdin, stdout) in [(false, true), (true, false), (false, false)] {
            let error = check_interactive(stdin, stdout).unwrap_err();
            assert!(error.to_string().contains("interactive terminal"));
        }
        assert!(check_interactive(true, true).is_ok());
    }

    #[test]
    fn restore_commands_leave_the_alternate_screen_and_show_the_cursor() {
        let mut buffer = Vec::new();
        write_restore_commands(&mut buffer).unwrap();
        let text = String::from_utf8(buffer).unwrap();
        assert!(
            text.contains("\u{1b}[?1049l"),
            "leave alternate screen: {text:?}"
        );
        assert!(text.contains("\u{1b}[?25h"), "show cursor: {text:?}");
    }

    #[test]
    fn emergency_restore_is_safe_without_a_terminal() {
        // Tests do not run in raw mode; every command here is best-effort.
        emergency_restore();
        emergency_restore();
    }

    #[test]
    fn panic_hook_restores_terminal_and_chains_to_the_previous_hook() {
        let _guard = HOOK_TEST_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        static CALLS: AtomicUsize = AtomicUsize::new(0);
        let original = panic::take_hook();
        panic::set_hook(Box::new(|_info| {
            CALLS.fetch_add(1, Ordering::SeqCst);
        }));
        install_panic_hook();
        let _ = panic::catch_unwind(|| panic!("tui hook test"));
        assert_eq!(CALLS.load(Ordering::SeqCst), 1);
        restore_panic_hook();
        let _ = panic::catch_unwind(|| panic!("tui hook test after restore"));
        assert_eq!(CALLS.load(Ordering::SeqCst), 2);
        panic::set_hook(original);
    }

    #[test]
    fn installing_and_restoring_twice_is_idempotent() {
        let _guard = HOOK_TEST_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let original = panic::take_hook();
        install_panic_hook();
        install_panic_hook();
        restore_panic_hook();
        restore_panic_hook();
        // A panic still uses the restored default chain.
        let _ = panic::catch_unwind(|| panic!("tui idempotent hook test"));
        panic::set_hook(original);
    }
}
