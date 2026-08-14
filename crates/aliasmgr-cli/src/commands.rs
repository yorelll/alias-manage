use crate::cli::Command;
use crate::messages::NON_INTERACTIVE;
use aliasmgr_core::error::AliasError;

pub fn requires_confirmation(command: &Command) -> bool { matches!(command, Command::Remove { yes: false, .. } | Command::Uninstall { purge_aliases: true }) }
pub fn ensure_interactive(command: &Command, is_tty: bool) -> Result<(), AliasError> {
    if requires_confirmation(command) && !is_tty { eprintln!("{NON_INTERACTIVE}"); return Err(AliasError::Config("NonInteractive".into())); }
    Ok(())
}
