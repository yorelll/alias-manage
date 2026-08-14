use crate::cli::{AddArgs, Command};
use crate::messages::NON_INTERACTIVE;
use aliasmgr_core::{error::AliasError, model::{AliasRecord, ShellKind}, storage::Database, validation::validate_alias};
use std::path::Path;

pub fn requires_confirmation(command: &Command) -> bool { matches!(command, Command::Remove { yes: false, .. } | Command::Uninstall { purge_aliases: true }) }
pub fn ensure_interactive(command: &Command, is_tty: bool) -> Result<(), AliasError> {
    if requires_confirmation(command) && !is_tty { eprintln!("{NON_INTERACTIVE}"); return Err(AliasError::Config("NonInteractive".into())); }
    Ok(())
}

fn database(config_dir: Option<&str>) -> Result<Database, AliasError> {
    let root = config_dir.map(Path::new).map(|path| path.to_path_buf()).or_else(|| std::env::var_os("ALIASMGR_CONFIG_DIR").map(Into::into)).unwrap_or_else(|| Path::new(".").join(".alias-manager").to_path_buf());
    std::fs::create_dir_all(&root)?;
    Database::open(root.join("aliases.db"))
}

pub fn add(config_dir: Option<&str>, args: &AddArgs) -> Result<AliasRecord, AliasError> {
    let mut alias = AliasRecord { name: args.name.clone(), executable: args.exec.clone(), fixed_args: args.args.clone(), pass_args: !args.no_pass_args, working_directory: args.cwd.clone(), tags: args.tags.clone(), ..Default::default() };
    if !args.shell.is_empty() { alias.shells = args.shell.iter().filter_map(|value| match value.as_str() { "bash" => Some(ShellKind::Bash), "zsh" => Some(ShellKind::Zsh), "powershell5" => Some(ShellKind::PowerShell5), "powershell7" => Some(ShellKind::PowerShell7), _ => None }).collect(); }
    validate_alias(&alias)?; let database = database(config_dir)?; database.insert_alias(&alias)?; Ok(alias)
}

pub fn get(config_dir: Option<&str>, name: &str) -> Result<Option<AliasRecord>, AliasError> { database(config_dir)?.get_alias_by_name(name) }
pub fn list(config_dir: Option<&str>) -> Result<Vec<AliasRecord>, AliasError> { database(config_dir)?.list_aliases() }
pub fn remove(config_dir: Option<&str>, name: &str) -> Result<bool, AliasError> { let database = database(config_dir)?; let alias = database.get_alias_by_name(name)?; match alias { Some(alias) => database.delete_alias(alias.id), None => Ok(false) } }
pub fn enable(config_dir: Option<&str>, name: &str, enabled: bool) -> Result<bool, AliasError> { let database = database(config_dir)?; let mut alias = database.get_alias_by_name(name)?.ok_or_else(|| AliasError::Config(format!("alias not found: {name}")))?; alias.enabled = enabled; database.update_alias(&alias) }
