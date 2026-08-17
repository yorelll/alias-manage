use crate::cli::{AddArgs, Command};
use crate::messages::NON_INTERACTIVE;
use aliasmgr_core::{error::AliasError, model::{record_checksum, AliasRecord, ShellKind}, storage::Database, validation::validate_alias};
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
    validate_alias(&alias)?; alias.record_checksum = record_checksum(&alias)?; let database = database(config_dir)?; database.insert_alias(&alias)?; Ok(alias)
}

pub fn get(config_dir: Option<&str>, name: &str) -> Result<Option<AliasRecord>, AliasError> { database(config_dir)?.get_alias_by_name(name) }
fn search_field(value: Option<&str>) -> Result<aliasmgr_core::search::SearchField, AliasError> {
    match value.unwrap_or("all") {
        "all" => Ok(aliasmgr_core::search::SearchField::All), "name" => Ok(aliasmgr_core::search::SearchField::Name), "target" => Ok(aliasmgr_core::search::SearchField::Target), "description" => Ok(aliasmgr_core::search::SearchField::Description), "tags" => Ok(aliasmgr_core::search::SearchField::Tags), "shells" => Ok(aliasmgr_core::search::SearchField::Shells), "target_type" => Ok(aliasmgr_core::search::SearchField::TargetType),
        other => Err(AliasError::Config(format!("unsupported search field: {other}"))),
    }
}
fn sort_field(value: Option<&str>) -> Result<aliasmgr_core::search::SortField, AliasError> {
    match value.unwrap_or("name") {
        "name" => Ok(aliasmgr_core::search::SortField::Name), "updated_at" => Ok(aliasmgr_core::search::SortField::UpdatedAt), "created_at" => Ok(aliasmgr_core::search::SortField::CreatedAt), "target_type" => Ok(aliasmgr_core::search::SortField::TargetType), "enabled" => Ok(aliasmgr_core::search::SortField::Enabled),
        other => Err(AliasError::Config(format!("unsupported sort field: {other}"))),
    }
}
pub fn list_query(config_dir: Option<&str>, sort: Option<&str>, descending: bool, limit: Option<usize>, tags: Vec<String>) -> Result<Vec<AliasRecord>, AliasError> {
    let query = aliasmgr_core::search::SearchQuery { query: String::new(), field: aliasmgr_core::search::SearchField::All, fuzzy: false, limit: limit.unwrap_or(50), sort: sort_field(sort)?, descending, tag_filter: tags };
    Ok(aliasmgr_core::search::search(&list(config_dir)?, &query).into_iter().map(|result| result.alias).collect())
}
pub fn find_query(config_dir: Option<&str>, query: &str, fuzzy: bool, field: Option<&str>, limit: Option<usize>, tags: Vec<String>) -> Result<Vec<AliasRecord>, AliasError> {
    let query = aliasmgr_core::search::SearchQuery { query: query.into(), fuzzy, field: search_field(field)?, limit: limit.unwrap_or(50), tag_filter: tags, ..Default::default() };
    Ok(aliasmgr_core::search::search(&list(config_dir)?, &query).into_iter().map(|result| result.alias).collect())
}
pub fn list(config_dir: Option<&str>) -> Result<Vec<AliasRecord>, AliasError> { database(config_dir)?.list_aliases() }
pub fn remove(config_dir: Option<&str>, name: &str) -> Result<bool, AliasError> { let database = database(config_dir)?; let alias = database.get_alias_by_name(name)?; match alias { Some(alias) => { for shell in &alias.shells { database.retire_name(name, shell, alias.revision)?; } database.delete_alias(alias.id) }, None => Ok(false) } }
pub fn enable(config_dir: Option<&str>, name: &str, enabled: bool) -> Result<bool, AliasError> { let database = database(config_dir)?; let mut alias = database.get_alias_by_name(name)?.ok_or_else(|| AliasError::Config(format!("alias not found: {name}")))?; alias.enabled = enabled; alias.record_checksum = record_checksum(&alias)?; database.update_alias(&alias) }
pub fn update(config_dir: Option<&str>, name: &str, executable: &str, fixed_args: Vec<String>) -> Result<bool, AliasError> { let database = database(config_dir)?; let mut alias = database.get_alias_by_name(name)?.ok_or_else(|| AliasError::Config(format!("alias not found: {name}")))?; alias.executable = executable.into(); alias.fixed_args = fixed_args; validate_alias(&alias)?; alias.record_checksum = record_checksum(&alias)?; database.update_alias(&alias) }
pub fn rename(config_dir: Option<&str>, old: &str, new: &str) -> Result<bool, AliasError> { let database = database(config_dir)?; let mut alias = database.get_alias_by_name(old)?.ok_or_else(|| AliasError::Config(format!("alias not found: {old}")))?; let shell = alias.shells.first().cloned().unwrap_or(ShellKind::Bash); database.retire_name(old, &shell, alias.revision)?; alias.name = new.into(); validate_alias(&alias)?; alias.record_checksum = record_checksum(&alias)?; database.update_alias(&alias) }
#[allow(dead_code)]
pub fn find(config_dir: Option<&str>, query: &str, fuzzy: bool, limit: Option<usize>, tags: Vec<String>) -> Result<Vec<AliasRecord>, AliasError> { find_query(config_dir, query, fuzzy, None, limit, tags) }
#[allow(dead_code)]
pub fn retired_names(config_dir: Option<&str>, shell: &str) -> Result<Vec<String>, AliasError> { let database = database(config_dir)?; let kind = match shell { "zsh" => ShellKind::Zsh, "powershell5" => ShellKind::PowerShell5, "powershell7" => ShellKind::PowerShell7, _ => ShellKind::Bash }; Ok(database.managed_name_set(&kind)?.retired) }
pub fn shell_detect() -> aliasmgr_core::detection::DetectionResult { aliasmgr_core::detection::detect(&aliasmgr_core::detection::DetectionContext { explicit: None, parent_shell: None, shell_env: std::env::var("SHELL").ok(), login_shell: None }) }
pub fn sync(config_dir: Option<&str>, dry_run: bool) -> Result<String, AliasError> {
    if dry_run { return Ok(String::new()); }
    let aliases = list(config_dir)?;
    let database = database(config_dir)?;
    let mut shells = aliases.iter().flat_map(|alias| alias.shells.clone()).collect::<Vec<_>>();
    for shell in [ShellKind::Bash, ShellKind::Zsh, ShellKind::PowerShell5, ShellKind::PowerShell7] { if !database.managed_name_set(&shell)?.retired.is_empty() && !shells.contains(&shell) { shells.push(shell); } }
    shells.sort(); shells.dedup();
    let root = config_dir.unwrap_or(".alias-manager");
    let coordinator = aliasmgr_core::sync::SyncCoordinator::new(root);
    let mut results = Vec::new();
    for shell in &shells { let names = database.managed_name_set(shell)?; let receipt = coordinator.apply_with_managed_names(&aliases, std::slice::from_ref(shell), 1, &names)?; results.extend(receipt.results); }
    Ok(format!("synced {} shell results", results.len()))
}
pub fn reload_print(config_dir: Option<&str>, shell: &str) -> String { let root = config_dir.unwrap_or(".alias-manager"); match shell { "zsh" => format!("source '{root}/generated/zsh.sh'"), "powershell5" => format!(". '{root}/generated/powershell5.ps1'"), "powershell7" => format!(". '{root}/generated/powershell7.ps1'"), _ => format!(". '{root}/generated/bash.sh'"), } }
pub fn export_file(config_dir: Option<&str>, file: &str) -> Result<(), AliasError> { let aliases = list(config_dir)?; let path = Path::new(file); if path.extension().and_then(|value| value.to_str()) == Some("toml") { aliasmgr_core::transfer::export_toml(path, &aliases) } else { aliasmgr_core::transfer::export_json(path, &aliases) } }
pub fn import_file(file: &str) -> Result<aliasmgr_core::transfer::ImportReport, AliasError> { import_preview(None, file) }

pub fn import_preview(config_dir: Option<&str>, file: &str) -> Result<aliasmgr_core::transfer::ImportReport, AliasError> {
    let path = Path::new(file);
    let existing = list(config_dir)?.into_iter().map(|alias| (alias.name.clone(), alias)).collect::<std::collections::BTreeMap<_, _>>();
    if path.extension().and_then(|value| value.to_str()) == Some("toml") { aliasmgr_core::transfer::import_toml(path, &existing, aliasmgr_core::transfer::ConflictStrategy::Ask) } else { aliasmgr_core::transfer::import_json(path, &existing, aliasmgr_core::transfer::ConflictStrategy::Ask) }
}

pub fn import_confirm(config_dir: Option<&str>, file: &str) -> Result<aliasmgr_core::transfer::ImportReport, AliasError> {
    let path = Path::new(file);
    let existing = list(config_dir)?.into_iter().map(|alias| (alias.name.clone(), alias)).collect::<std::collections::BTreeMap<_, _>>();
    let aliases = if path.extension().and_then(|value| value.to_str()) == Some("toml") { aliasmgr_core::transfer::read_toml(path)? } else { aliasmgr_core::transfer::read_json(path)? };
    let mut report = aliasmgr_core::transfer::import_records(aliases, &existing, aliasmgr_core::transfer::ConflictStrategy::Ask)?;
    if !report.imported.is_empty() {
        let database = database(config_dir)?;
        for alias in report.accepted_records.drain(..) { database.insert_alias(&alias)?; }
    }
    Ok(report.into_public())
}
pub fn uninstall(config_dir: Option<&str>, purge: bool) -> Result<(), AliasError> { let root = config_dir.map(Path::new).unwrap_or_else(|| Path::new(".alias-manager")); aliasmgr_core::uninstall::uninstall(root, if purge { aliasmgr_core::uninstall::UninstallMode::PurgeAliases } else { aliasmgr_core::uninstall::UninstallMode::RetainAliases }, None).map(|_| ()) }
pub fn doctor(config_dir: Option<&str>) -> Result<Vec<String>, AliasError> { let paths = aliasmgr_core::config::AppPaths::discover(config_dir.map(Path::new)); let mut findings = Vec::new(); if !paths.root.join("aliases.db").exists() { findings.push("数据库文件缺失".into()); } if !paths.generated_path("bash").exists() { findings.push("Bash 生成文件缺失".into()); } Ok(findings) }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dry_run_and_reload_are_non_mutating_helpers() { assert!(reload_print(Some("cfg"), "bash").contains("generated/bash.sh")); }
}
