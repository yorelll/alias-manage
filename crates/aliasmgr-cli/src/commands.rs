use crate::cli::{AddArgs, Command, UpdateArgs};
use crate::messages::NON_INTERACTIVE;
use aliasmgr_core::transfer::{ConflictStrategy, ImportReport};
use aliasmgr_core::{error::AliasError, model::{record_checksum, AliasRecord, ShellKind}, storage::Database, validation::validate_alias};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub fn requires_confirmation(command: &Command) -> bool { matches!(command, Command::Remove { yes: false, .. } | Command::Uninstall { purge_aliases: true, yes: false }) }
pub fn ensure_interactive(command: &Command, is_tty: bool) -> Result<(), AliasError> {
    if requires_confirmation(command) && !is_tty { eprintln!("{NON_INTERACTIVE}"); return Err(AliasError::ConfirmationRequired); }
    Ok(())
}

fn config_root(config_dir: Option<&str>) -> PathBuf {
    config_dir.map(Path::new).map(|path| path.to_path_buf()).or_else(|| std::env::var_os("ALIASMGR_CONFIG_DIR").map(Into::into)).unwrap_or_else(|| Path::new(".").join(".alias-manager").to_path_buf())
}

fn database(config_dir: Option<&str>) -> Result<Database, AliasError> {
    let root = config_root(config_dir);
    std::fs::create_dir_all(&root)?;
    Database::open(root.join("aliases.db"))
}

fn shell_kind(value: &str) -> Option<ShellKind> {
    match value.to_ascii_lowercase().as_str() {
        "bash" => Some(ShellKind::Bash),
        "zsh" => Some(ShellKind::Zsh),
        "powershell5" => Some(ShellKind::PowerShell5),
        "powershell7" => Some(ShellKind::PowerShell7),
        _ => None,
    }
}

fn parse_shells(values: &[String]) -> Result<Vec<ShellKind>, AliasError> {
    values.iter().map(|value| shell_kind(value).ok_or_else(|| AliasError::Config(format!("unsupported shell: {value}")))).collect()
}

fn parse_environment(values: &[String]) -> Result<BTreeMap<String, String>, AliasError> {
    let mut environment = BTreeMap::new();
    for value in values {
        let (key, item) = value.split_once('=').ok_or_else(|| AliasError::Config(format!("invalid environment entry (expected KEY=VALUE): {value}")))?;
        if key.is_empty() { return Err(AliasError::Config(format!("invalid environment entry (empty key): {value}"))); }
        environment.insert(key.to_string(), item.to_string());
    }
    Ok(environment)
}

pub fn add(config_dir: Option<&str>, args: &AddArgs) -> Result<AliasRecord, AliasError> {
    let mut alias = AliasRecord { name: args.name.clone(), executable: args.exec.clone(), fixed_args: args.args.clone(), pass_args: !args.no_pass_args, working_directory: args.cwd.clone(), environment: parse_environment(&args.environment)?, tags: args.tags.clone(), ..Default::default() };
    if !args.shell.is_empty() { alias.shells = parse_shells(&args.shell)?; }
    validate_alias(&alias)?; alias.record_checksum = record_checksum(&alias)?; database(config_dir)?.insert_alias(&alias)?; Ok(alias)
}
pub fn get(config_dir: Option<&str>, name: &str) -> Result<AliasRecord, AliasError> { database(config_dir)?.get_alias_by_name(name)?.ok_or_else(|| AliasError::NotFound(name.into())) }
pub fn list(config_dir: Option<&str>) -> Result<Vec<AliasRecord>, AliasError> { database(config_dir)?.list_aliases() }
fn search_field(value: Option<&str>) -> Result<aliasmgr_core::search::SearchField, AliasError> { match value.unwrap_or("all") { "all" => Ok(aliasmgr_core::search::SearchField::All), "name" => Ok(aliasmgr_core::search::SearchField::Name), "target" => Ok(aliasmgr_core::search::SearchField::Target), "description" => Ok(aliasmgr_core::search::SearchField::Description), "tags" => Ok(aliasmgr_core::search::SearchField::Tags), "shells" => Ok(aliasmgr_core::search::SearchField::Shells), "target_type" => Ok(aliasmgr_core::search::SearchField::TargetType), other => Err(AliasError::Config(format!("unsupported search field: {other}"))) } }
fn sort_field(value: Option<&str>) -> Result<aliasmgr_core::search::SortField, AliasError> { match value.unwrap_or("name") { "name" => Ok(aliasmgr_core::search::SortField::Name), "updated_at" => Ok(aliasmgr_core::search::SortField::UpdatedAt), "created_at" => Ok(aliasmgr_core::search::SortField::CreatedAt), "target_type" => Ok(aliasmgr_core::search::SortField::TargetType), "enabled" => Ok(aliasmgr_core::search::SortField::Enabled), other => Err(AliasError::Config(format!("unsupported sort field: {other}"))) } }
pub fn list_query(config_dir: Option<&str>, sort: Option<&str>, descending: bool, limit: Option<usize>, tags: Vec<String>) -> Result<Vec<AliasRecord>, AliasError> { let query = aliasmgr_core::search::SearchQuery { query: String::new(), field: aliasmgr_core::search::SearchField::All, fuzzy: false, limit: limit.unwrap_or(50), sort: sort_field(sort)?, descending, tag_filter: tags }; Ok(aliasmgr_core::search::search(&list(config_dir)?, &query).into_iter().map(|result| result.alias).collect()) }
pub fn find_query(config_dir: Option<&str>, query: &str, fuzzy: bool, field: Option<&str>, limit: Option<usize>, tags: Vec<String>) -> Result<Vec<AliasRecord>, AliasError> { let query = aliasmgr_core::search::SearchQuery { query: query.into(), fuzzy, field: search_field(field)?, limit: limit.unwrap_or(50), tag_filter: tags, ..Default::default() }; Ok(aliasmgr_core::search::search(&list(config_dir)?, &query).into_iter().map(|result| result.alias).collect()) }
pub fn remove(config_dir: Option<&str>, name: &str) -> Result<bool, AliasError> {
    let database = database(config_dir)?;
    let alias = database.get_alias_by_name(name)?.ok_or_else(|| AliasError::NotFound(name.into()))?;
    for shell in &alias.shells { database.retire_name(name, shell, alias.revision)?; }
    database.delete_alias(alias.id)
}
pub fn enable(config_dir: Option<&str>, name: &str, enabled: bool) -> Result<bool, AliasError> { let database = database(config_dir)?; let mut alias = database.get_alias_by_name(name)?.ok_or_else(|| AliasError::NotFound(name.into()))?; alias.enabled = enabled; alias.record_checksum = record_checksum(&alias)?; database.update_alias(&alias) }
pub fn update(config_dir: Option<&str>, args: &UpdateArgs) -> Result<AliasRecord, AliasError> {
    let database = database(config_dir)?;
    let mut alias = database.get_alias_by_name(&args.name)?.ok_or_else(|| AliasError::NotFound(args.name.clone()))?;
    let previous_shells = alias.shells.clone();
    if let Some(executable) = &args.exec { alias.executable = executable.clone(); }
    if let Some(fixed_args) = &args.args { alias.fixed_args = fixed_args.clone(); }
    if let Some(shells) = &args.shell { alias.shells = parse_shells(shells)?; }
    if let Some(working_directory) = &args.cwd { alias.working_directory = Some(working_directory.clone()); }
    if let Some(environment) = &args.environment { for (key, value) in parse_environment(environment)? { alias.environment.insert(key, value); } }
    if let Some(tags) = &args.tags { alias.tags = tags.clone(); }
    if args.no_pass_args { alias.pass_args = false; }
    if args.pass_args { alias.pass_args = true; }
    validate_alias(&alias)?;
    for shell in previous_shells.iter().filter(|shell| !alias.shells.contains(shell)) {
        database.retire_name(&alias.name, shell, alias.revision)?;
    }
    alias.record_checksum = record_checksum(&alias)?;
    database.update_alias(&alias)?;
    Ok(alias)
}
pub fn rename(config_dir: Option<&str>, old: &str, new: &str) -> Result<bool, AliasError> { let database = database(config_dir)?; let mut alias = database.get_alias_by_name(old)?.ok_or_else(|| AliasError::NotFound(old.into()))?; let shell = alias.shells.first().cloned().unwrap_or(ShellKind::Bash); database.retire_name(old, &shell, alias.revision)?; alias.name = new.into(); validate_alias(&alias)?; alias.record_checksum = record_checksum(&alias)?; database.update_alias(&alias) }
#[allow(dead_code)] pub fn find(config_dir: Option<&str>, query: &str, fuzzy: bool, limit: Option<usize>, tags: Vec<String>) -> Result<Vec<AliasRecord>, AliasError> { find_query(config_dir, query, fuzzy, None, limit, tags) }
#[allow(dead_code)] pub fn retired_names(config_dir: Option<&str>, shell: &str) -> Result<Vec<String>, AliasError> { let kind = match shell { "zsh" => ShellKind::Zsh, "powershell5" => ShellKind::PowerShell5, "powershell7" => ShellKind::PowerShell7, _ => ShellKind::Bash }; Ok(database(config_dir)?.managed_name_set(&kind)?.retired) }
pub fn shell_detect() -> aliasmgr_core::detection::DetectionResult { aliasmgr_core::detection::detect(&aliasmgr_core::detection::DetectionContext { explicit: None, parent_shell: None, shell_env: std::env::var("SHELL").ok(), login_shell: None }) }
pub fn shell_cli_name(kind: &ShellKind) -> &'static str { match kind { ShellKind::Bash => "bash", ShellKind::Zsh => "zsh", ShellKind::PowerShell5 => "powershell5", ShellKind::PowerShell7 => "powershell7", ShellKind::Fish => "fish", ShellKind::PosixSh => "sh" } }
pub fn detected_shell_name() -> String { shell_detect().selected.map(|kind| shell_cli_name(&kind).to_string()).unwrap_or_else(|| "bash".to_string()) }
pub fn reload_shell(explicit: Option<&str>) -> Result<String, AliasError> {
    match explicit {
        Some(value) => shell_kind(value).map(|kind| shell_cli_name(&kind).to_string()).ok_or_else(|| AliasError::Config(format!("unsupported shell: {value}"))),
        None => Ok(detected_shell_name()),
    }
}
pub fn sync(config_dir: Option<&str>, dry_run: bool) -> Result<String, AliasError> { if dry_run { return Ok(String::new()); } let aliases = list(config_dir)?; let database = database(config_dir)?; let mut shells = aliases.iter().flat_map(|alias| alias.shells.clone()).collect::<Vec<_>>(); for shell in [ShellKind::Bash, ShellKind::Zsh, ShellKind::PowerShell5, ShellKind::PowerShell7] { if !database.managed_name_set(&shell)?.retired.is_empty() && !shells.contains(&shell) { shells.push(shell); } } shells.sort(); shells.dedup(); let coordinator = aliasmgr_core::sync::SyncCoordinator::new(config_dir.unwrap_or(".alias-manager")); let mut results = Vec::new(); for shell in &shells { let names = database.managed_name_set(shell)?; let shell_aliases = aliases.iter().filter(|alias| alias.shells.contains(shell)).cloned().collect::<Vec<_>>(); results.extend(coordinator.apply_with_managed_names(&shell_aliases, std::slice::from_ref(shell), 1, &names)?.results); } Ok(format!("synced {} shell results", results.len())) }
pub fn reload_print(config_dir: Option<&str>, shell: &str) -> String { let root = config_dir.unwrap_or(".alias-manager"); match shell { "zsh" => format!("source '{root}/generated/zsh.sh'"), "powershell5" => format!(". '{root}/generated/powershell5.ps1'"), "powershell7" => format!(". '{root}/generated/powershell7.ps1'"), _ => format!(". '{root}/generated/bash.sh'"), } }

pub fn shell_install(config_dir: Option<&str>, shell: &str) -> Result<bool, AliasError> {
    let paths = aliasmgr_core::config::AppPaths::discover(config_dir.map(Path::new));
    let config = paths.load_config()?;
    let (rc_path, generated) = match shell.to_ascii_lowercase().as_str() { "bash" => (config.shells.bash_rc_path.unwrap_or_else(|| paths.root.join(".bashrc")), paths.generated_path("bash")), "zsh" => (config.shells.zsh_rc_path.unwrap_or_else(|| paths.root.join(".zshrc")), paths.generated_path("zsh")), _ => return Err(AliasError::ShellNotInstalled) };
    let loader = aliasmgr_core::shells::common::render_loader(&generated);
    let before = std::fs::read_to_string(&rc_path).unwrap_or_default();
    if aliasmgr_core::shells::common::install_loader(&before, &loader)? == before { return Ok(false); }
    aliasmgr_core::shells::common::write_loader_file(&rc_path, &before, &loader)?;
    Ok(true)
}

pub fn shell_uninstall(config_dir: Option<&str>, shell: &str) -> Result<bool, AliasError> {
    let paths = aliasmgr_core::config::AppPaths::discover(config_dir.map(Path::new));
    let config = paths.load_config()?;
    let rc_path = match shell.to_ascii_lowercase().as_str() { "bash" => config.shells.bash_rc_path.unwrap_or_else(|| paths.root.join(".bashrc")), "zsh" => config.shells.zsh_rc_path.unwrap_or_else(|| paths.root.join(".zshrc")), _ => return Err(AliasError::ShellNotInstalled) };
    if !rc_path.exists() { return Ok(false); }
    let before = std::fs::read_to_string(&rc_path)?;
    let cleaned = aliasmgr_core::shells::common::remove_loader(&before)?;
    if cleaned == before { return Ok(false); }
    aliasmgr_core::shells::common::write_profile_content(&rc_path, &cleaned)?;
    Ok(true)
}

#[allow(dead_code)]
pub fn shell_install_status(config_dir: Option<&str>, shell: &str) -> Result<bool, AliasError> {
    let paths = aliasmgr_core::config::AppPaths::discover(config_dir.map(Path::new));
    let config = paths.load_config()?;
    let rc_path = match shell.to_ascii_lowercase().as_str() { "bash" => config.shells.bash_rc_path.unwrap_or_else(|| paths.root.join(".bashrc")), "zsh" => config.shells.zsh_rc_path.unwrap_or_else(|| paths.root.join(".zshrc")), _ => return Err(AliasError::ShellNotInstalled) };
    Ok(rc_path.exists() && aliasmgr_core::shells::common::loader_position(&std::fs::read_to_string(rc_path)?)? == aliasmgr_core::shells::common::LoaderPosition::AtEnd)
}


pub fn export_file(config_dir: Option<&str>, file: &str) -> Result<(), AliasError> { let aliases = list(config_dir)?; let path = Path::new(file); if path.extension().and_then(|value| value.to_str()) == Some("toml") { aliasmgr_core::transfer::export_toml(path, &aliases) } else { aliasmgr_core::transfer::export_json(path, &aliases) } }

fn read_import_file(file: &str) -> Result<Vec<AliasRecord>, AliasError> {
    let path = Path::new(file);
    if path.extension().and_then(|value| value.to_str()) == Some("toml") { aliasmgr_core::transfer::read_toml(path) } else { aliasmgr_core::transfer::read_json(path) }
}

fn existing_aliases(config_dir: Option<&str>) -> Result<BTreeMap<String, AliasRecord>, AliasError> {
    let path = config_root(config_dir).join("aliases.db");
    if !path.exists() { return Ok(BTreeMap::new()); }
    Ok(Database::open_read_only(path)?.list_aliases()?.into_iter().map(|alias| (alias.name.clone(), alias)).collect())
}

pub fn import_file(config_dir: Option<&str>, file: &str, confirm: bool) -> Result<ImportReport, AliasError> {
    if confirm { import_confirm(config_dir, file) } else { import_preview(config_dir, file) }
}

pub fn import_preview(config_dir: Option<&str>, file: &str) -> Result<ImportReport, AliasError> {
    let existing = existing_aliases(config_dir)?;
    aliasmgr_core::transfer::import_records(read_import_file(file)?, &existing, ConflictStrategy::Ask).map(|report| report.into_public())
}

pub fn import_confirm(config_dir: Option<&str>, file: &str) -> Result<ImportReport, AliasError> {
    let existing = list(config_dir)?.into_iter().map(|alias| (alias.name.clone(), alias)).collect();
    let report = aliasmgr_core::transfer::import_records(read_import_file(file)?, &existing, ConflictStrategy::Ask)?;
    database(config_dir)?.insert_aliases_atomic(&report.accepted_records)?;
    Ok(report.into_public())
}
pub fn uninstall(config_dir: Option<&str>, purge: bool) -> Result<(), AliasError> { let root = config_dir.map(Path::new).unwrap_or_else(|| Path::new(".alias-manager")); aliasmgr_core::uninstall::uninstall(root, if purge { aliasmgr_core::uninstall::UninstallMode::PurgeAliases } else { aliasmgr_core::uninstall::UninstallMode::RetainAliases }, None).map(|_| ()) }
pub fn doctor(config_dir: Option<&str>) -> Result<Vec<String>, AliasError> {
    let paths = aliasmgr_core::config::AppPaths::discover(config_dir.map(Path::new));
    let database_path_exists = paths.root.join("aliases.db").exists();
    let mut findings = Vec::new();
    if !database_path_exists { findings.push("数据库文件缺失".into()); }
    let database = database(config_dir)?;
    for shell in [ShellKind::Bash, ShellKind::Zsh, ShellKind::PowerShell5, ShellKind::PowerShell7] {
        if !paths.generated_path(match shell { ShellKind::Bash => "bash", ShellKind::Zsh => "zsh", ShellKind::PowerShell5 => "powershell5", ShellKind::PowerShell7 => "powershell7", _ => "unsupported" }).exists() { findings.push(format!("生成文件缺失: {shell:?}")); }
    }
    let revision = database.list_aliases()?.iter().map(|alias| alias.revision).max().unwrap_or(0);
    for state in database.shell_states()? {
        if state.applied_revision < revision { findings.push(format!("Shell {:?} 生成文件过期", state.shell)); }
        if let Some(error) = state.last_error { findings.push(format!("Shell {:?} 同步失败: {error}", state.shell)); }
        if !state.loader_installed { findings.push(format!("Shell {:?} 加载块未安装", state.shell)); }
    }
    if let Some(error) = database.reliability_error() { findings.push(error.to_string()); }
    Ok(findings)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn dry_run_and_reload_are_non_mutating_helpers() { assert!(reload_print(Some("cfg"), "bash").contains("generated/bash.sh")); }
    #[test]
    fn reload_shell_honours_explicit_choice_and_detection_stays_within_known_names() {
        assert_eq!(reload_shell(Some("zsh")).unwrap(), "zsh");
        assert_eq!(reload_shell(Some("ZSH")).unwrap(), "zsh");
        assert!(matches!(reload_shell(Some("fish")), Err(AliasError::Config(message)) if message.contains("unsupported shell: fish")));
        assert!(["bash", "zsh", "powershell5", "powershell7"].contains(&reload_shell(None).unwrap().as_str()));
        assert!(reload_print(Some("cfg"), "zsh").contains("generated/zsh.sh"));
        assert!(reload_print(Some("cfg"), "powershell5").contains("generated/powershell5.ps1"));
        assert!(reload_print(Some("cfg"), "powershell7").contains("generated/powershell7.ps1"));
    }

    #[test]
    fn ensure_interactive_requires_confirmation_only_without_yes() {
        let remove = Command::Remove { name: "gs".into(), yes: false };
        assert!(matches!(ensure_interactive(&remove, false), Err(AliasError::ConfirmationRequired)));
        assert!(ensure_interactive(&remove, true).is_ok());
        assert!(ensure_interactive(&Command::Remove { name: "gs".into(), yes: true }, false).is_ok());

        let purge = Command::Uninstall { purge_aliases: true, yes: false };
        assert!(requires_confirmation(&purge));
        assert!(matches!(ensure_interactive(&purge, false), Err(AliasError::ConfirmationRequired)));
        assert!(ensure_interactive(&Command::Uninstall { purge_aliases: true, yes: true }, false).is_ok());
        assert!(ensure_interactive(&Command::Uninstall { purge_aliases: false, yes: false }, false).is_ok());
    }
}
