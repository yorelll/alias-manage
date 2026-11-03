use crate::cli::{AddArgs, Command, RestrictionModeArg, UpdateArgs};
use crate::messages::NON_INTERACTIVE;
use aliasmgr_core::transfer::{ConflictStrategy, ImportReport};
use aliasmgr_core::{
    config::AppPaths,
    detection,
    error::AliasError,
    model::{
        normalize_restriction, record_checksum, AliasRecord, PathRestriction, ShellKind,
        SUPPORTED_SHELLS,
    },
    shell_setup,
    storage::Database,
    validation::validate_alias,
};
use std::collections::BTreeMap;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

pub fn requires_confirmation(command: &Command) -> bool {
    matches!(
        command,
        Command::Remove { yes: false, .. }
            | Command::Uninstall {
                purge_aliases: true,
                yes: false
            }
    )
}
pub fn ensure_interactive(command: &Command, is_tty: bool) -> Result<(), AliasError> {
    if requires_confirmation(command) && !is_tty {
        eprintln!("{NON_INTERACTIVE}");
        return Err(AliasError::ConfirmationRequired);
    }
    Ok(())
}

fn database(config_dir: Option<&str>) -> Result<Database, AliasError> {
    let root = AppPaths::discover(config_dir.map(Path::new)).root;
    std::fs::create_dir_all(&root)?;
    Database::open(root.join("aliases.db"))
}

fn paths_for(config_dir: Option<&str>) -> AppPaths {
    AppPaths::discover(config_dir.map(Path::new))
}

fn canonical_supported(shells: &[ShellKind]) -> Vec<ShellKind> {
    SUPPORTED_SHELLS
        .iter()
        .filter(|kind| shells.contains(kind))
        .cloned()
        .collect()
}

/// Refuses to save an alias whose target shells are not ready, unless the
/// caller explicitly opted out with `--allow-missing-loader`. With
/// `skip_loader` (the `--no-sync` case) the enabled check still runs but the
/// loader check is deferred to the user's later sync.
fn preflight_target_shells(
    config_dir: Option<&str>,
    shells: &[ShellKind],
    allow_missing_loader: bool,
    skip_loader: bool,
) -> Result<(), AliasError> {
    let paths = paths_for(config_dir);
    let config = paths.load_config()?;
    // Shared rule with the GUI: enabled intent plus (unless deferred) a
    // verifiable loader block. Canonical shell order and unsupported filtering
    // also live in core, so the two front ends cannot drift.
    let problems =
        shell_setup::shell_readiness_problems_with(&config, &paths, shells, !skip_loader)?;
    if problems.is_empty() {
        return Ok(());
    }
    let detail = problems
        .iter()
        .map(|problem| problem.message.as_str())
        .collect::<Vec<_>>()
        .join("; ");
    if allow_missing_loader {
        eprintln!(
            "warning: saving to the database, but the alias will not take effect yet: {detail}. Run `aliasmgr doctor`."
        );
        return Ok(());
    }
    Err(AliasError::ShellNotReady(format!(
        "{detail}. Run `aliasmgr doctor` to enable shells and install integration, or pass --allow-missing-loader to save anyway."
    )))
}

/// Syncs the shells affected by a successful database write once. A no-op when
/// `--no-sync` was passed, `runtime.auto_sync = false`, or no supported shell
/// is affected. Any sync failure after the database commit is surfaced as
/// `PartialSync` (exit 10) so scripts do not mistake it for a lost write.
fn sync_after_write(
    config_dir: Option<&str>,
    affected: &[ShellKind],
    no_sync: bool,
) -> Result<(), AliasError> {
    if no_sync {
        return Ok(());
    }
    let shells = canonical_supported(affected);
    if shells.is_empty() {
        return Ok(());
    }
    let paths = paths_for(config_dir);
    let config = match paths.load_config() {
        Ok(config) => config,
        Err(error) => {
            return Err(AliasError::PartialSync(format!(
                "database changes were saved, but the configuration could not be read: {error}"
            )))
        }
    };
    if !config.runtime.auto_sync {
        return Ok(());
    }
    let run = aliasmgr_core::sync::sync_shells(&paths.root, &shells).map_err(|error| {
        AliasError::PartialSync(format!(
            "database changes were saved, but sync failed: {error}. Run `aliasmgr sync` or `aliasmgr doctor`."
        ))
    })?;
    let failures = run.failure_summary();
    if failures.is_empty() {
        Ok(())
    } else {
        Err(AliasError::PartialSync(format!(
            "database changes were saved, but these shells failed to sync: {}. Run `aliasmgr sync` or `aliasmgr doctor`.",
            failures.join("; ")
        )))
    }
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
    values
        .iter()
        .map(|value| {
            shell_kind(value)
                .ok_or_else(|| AliasError::Config(format!("unsupported shell: {value}")))
        })
        .collect()
}

fn parse_environment(values: &[String]) -> Result<BTreeMap<String, String>, AliasError> {
    let mut environment = BTreeMap::new();
    for value in values {
        let (key, item) = value.split_once('=').ok_or_else(|| {
            AliasError::Config(format!(
                "invalid environment entry (expected KEY=VALUE): {value}"
            ))
        })?;
        if key.is_empty() {
            return Err(AliasError::Config(format!(
                "invalid environment entry (empty key): {value}"
            )));
        }
        environment.insert(key.to_string(), item.to_string());
    }
    Ok(environment)
}

fn trimmed_restriction_paths(paths: &[String]) -> Vec<String> {
    paths.iter().map(|path| path.trim().to_string()).collect()
}

fn new_alias_restriction(
    mode: Option<RestrictionModeArg>,
    paths: &[String],
) -> Result<Option<PathRestriction>, AliasError> {
    let paths = trimmed_restriction_paths(paths);
    match mode {
        Some(mode) => {
            if paths.is_empty() {
                return Err(AliasError::Config(
                    "--restrict requires at least one --restrict-path".into(),
                ));
            }
            Ok(Some(PathRestriction {
                mode: mode.into(),
                paths,
            }))
        }
        None => {
            if !paths.is_empty() {
                return Err(AliasError::Config(
                    "--restrict-path requires --restrict".into(),
                ));
            }
            Ok(None)
        }
    }
}

fn apply_update_restriction(
    alias: &mut AliasRecord,
    mode: Option<RestrictionModeArg>,
    raw_paths: &[String],
    clear: bool,
) -> Result<(), AliasError> {
    if clear {
        alias.restriction = None;
        return Ok(());
    }
    let paths = trimmed_restriction_paths(raw_paths);
    match (mode, paths.is_empty()) {
        (Some(mode), false) => {
            alias.restriction = Some(PathRestriction {
                mode: mode.into(),
                paths,
            });
        }
        (Some(mode), true) => {
            match alias.restriction.as_ref() {
                Some(existing) => {
                    alias.restriction = Some(PathRestriction {
                        mode: mode.into(),
                        paths: existing.paths.clone(),
                    });
                }
                None => return Err(AliasError::Config(
                    "--restrict requires at least one --restrict-path when no restriction exists"
                        .into(),
                )),
            }
        }
        (None, false) => match alias.restriction.as_mut() {
            Some(existing) => existing.paths = paths,
            None => {
                return Err(AliasError::Config(
                    "--restrict-path requires an existing restriction; pass --restrict as well"
                        .into(),
                ))
            }
        },
        (None, true) => {}
    }
    Ok(())
}

pub fn add(config_dir: Option<&str>, args: &AddArgs) -> Result<AliasRecord, AliasError> {
    let mut alias = AliasRecord {
        name: args.name.clone(),
        description: args.description.clone().unwrap_or_default(),
        executable: args.exec.clone(),
        fixed_args: args.args.clone(),
        pass_args: !args.no_pass_args,
        working_directory: args.cwd.clone(),
        environment: parse_environment(&args.environment)?,
        tags: args.tags.clone(),
        restriction: new_alias_restriction(args.restrict, &args.restrict_paths)?,
        ..Default::default()
    };
    if let Some(target_type) = args.target_type {
        alias.target_type = target_type.into();
    }
    if let Some(path_mode) = args.path_mode {
        alias.path_mode = path_mode.into();
    }
    if !args.shell.is_empty() {
        alias.shells = parse_shells(&args.shell)?;
    }
    normalize_restriction(&mut alias.restriction);
    validate_alias(&alias)?;
    preflight_target_shells(
        config_dir,
        &alias.shells,
        args.allow_missing_loader,
        args.no_sync,
    )?;
    alias.record_checksum = record_checksum(&alias)?;
    database(config_dir)?.insert_alias(&alias)?;
    sync_after_write(config_dir, &alias.shells, args.no_sync)?;
    Ok(alias)
}
pub fn get(config_dir: Option<&str>, name: &str) -> Result<AliasRecord, AliasError> {
    database(config_dir)?
        .get_alias_by_name(name)?
        .ok_or_else(|| AliasError::NotFound(name.into()))
}
pub fn list(config_dir: Option<&str>) -> Result<Vec<AliasRecord>, AliasError> {
    database(config_dir)?.list_aliases()
}
fn search_field(value: Option<&str>) -> Result<aliasmgr_core::search::SearchField, AliasError> {
    match value.unwrap_or("all") {
        "all" => Ok(aliasmgr_core::search::SearchField::All),
        "name" => Ok(aliasmgr_core::search::SearchField::Name),
        "target" => Ok(aliasmgr_core::search::SearchField::Target),
        "description" => Ok(aliasmgr_core::search::SearchField::Description),
        "tags" => Ok(aliasmgr_core::search::SearchField::Tags),
        "shells" => Ok(aliasmgr_core::search::SearchField::Shells),
        "target_type" => Ok(aliasmgr_core::search::SearchField::TargetType),
        other => Err(AliasError::Config(format!(
            "unsupported search field: {other}"
        ))),
    }
}
fn sort_field(value: Option<&str>) -> Result<aliasmgr_core::search::SortField, AliasError> {
    match value.unwrap_or("name") {
        "name" => Ok(aliasmgr_core::search::SortField::Name),
        "updated_at" => Ok(aliasmgr_core::search::SortField::UpdatedAt),
        "created_at" => Ok(aliasmgr_core::search::SortField::CreatedAt),
        "target_type" => Ok(aliasmgr_core::search::SortField::TargetType),
        "enabled" => Ok(aliasmgr_core::search::SortField::Enabled),
        other => Err(AliasError::Config(format!(
            "unsupported sort field: {other}"
        ))),
    }
}
pub fn list_query(
    config_dir: Option<&str>,
    sort: Option<&str>,
    descending: bool,
    limit: Option<usize>,
    tags: Vec<String>,
) -> Result<Vec<AliasRecord>, AliasError> {
    let query = aliasmgr_core::search::SearchQuery {
        query: String::new(),
        field: aliasmgr_core::search::SearchField::All,
        fuzzy: false,
        limit: limit.unwrap_or(50),
        sort: sort_field(sort)?,
        descending,
        tag_filter: tags,
    };
    Ok(aliasmgr_core::search::search(&list(config_dir)?, &query)
        .into_iter()
        .map(|result| result.alias)
        .collect())
}
pub fn find_query(
    config_dir: Option<&str>,
    query: &str,
    fuzzy: bool,
    field: Option<&str>,
    limit: Option<usize>,
    tags: Vec<String>,
) -> Result<Vec<AliasRecord>, AliasError> {
    let query = aliasmgr_core::search::SearchQuery {
        query: query.into(),
        fuzzy,
        field: search_field(field)?,
        limit: limit.unwrap_or(50),
        tag_filter: tags,
        ..Default::default()
    };
    Ok(aliasmgr_core::search::search(&list(config_dir)?, &query)
        .into_iter()
        .map(|result| result.alias)
        .collect())
}
#[allow(dead_code)]
pub fn remove(config_dir: Option<&str>, name: &str) -> Result<bool, AliasError> {
    remove_with_sync(config_dir, name, false)
}
pub fn remove_with_sync(
    config_dir: Option<&str>,
    name: &str,
    no_sync: bool,
) -> Result<bool, AliasError> {
    let database = database(config_dir)?;
    let alias = database
        .get_alias_by_name(name)?
        .ok_or_else(|| AliasError::NotFound(name.into()))?;
    let shells = alias.shells.clone();
    for shell in &alias.shells {
        database.retire_name(name, shell, alias.revision.saturating_add(1))?;
    }
    let removed = database.delete_alias(alias.id)?;
    if removed {
        sync_after_write(config_dir, &shells, no_sync)?;
    }
    Ok(removed)
}
#[allow(dead_code)]
pub fn enable(config_dir: Option<&str>, name: &str, enabled: bool) -> Result<bool, AliasError> {
    enable_with_sync(config_dir, name, enabled, false)
}
pub fn enable_with_sync(
    config_dir: Option<&str>,
    name: &str,
    enabled: bool,
    no_sync: bool,
) -> Result<bool, AliasError> {
    let database = database(config_dir)?;
    let mut alias = database
        .get_alias_by_name(name)?
        .ok_or_else(|| AliasError::NotFound(name.into()))?;
    alias.enabled = enabled;
    alias.record_checksum = record_checksum(&alias)?;
    let changed = database.update_alias(&alias)?;
    if changed {
        sync_after_write(config_dir, &alias.shells, no_sync)?;
    }
    Ok(changed)
}
pub fn update(config_dir: Option<&str>, args: &UpdateArgs) -> Result<AliasRecord, AliasError> {
    let database = database(config_dir)?;
    let mut alias = database
        .get_alias_by_name(&args.name)?
        .ok_or_else(|| AliasError::NotFound(args.name.clone()))?;
    let previous_shells = alias.shells.clone();
    if let Some(executable) = &args.exec {
        alias.executable = executable.clone();
    }
    if let Some(fixed_args) = &args.args {
        alias.fixed_args = fixed_args.clone();
    }
    if let Some(shells) = &args.shell {
        alias.shells = parse_shells(shells)?;
    }
    if let Some(working_directory) = &args.cwd {
        alias.working_directory = Some(working_directory.clone());
    }
    if let Some(environment) = &args.environment {
        for (key, value) in parse_environment(environment)? {
            alias.environment.insert(key, value);
        }
    }
    if let Some(tags) = &args.tags {
        alias.tags = tags.clone();
    }
    if let Some(description) = &args.description {
        alias.description = description.clone();
    }
    apply_update_restriction(
        &mut alias,
        args.restrict,
        &args.restrict_paths,
        args.clear_restriction,
    )?;
    normalize_restriction(&mut alias.restriction);
    if let Some(target_type) = args.target_type {
        alias.target_type = target_type.into();
    }
    if let Some(path_mode) = args.path_mode {
        alias.path_mode = path_mode.into();
    }
    if args.no_pass_args {
        alias.pass_args = false;
    }
    if args.pass_args {
        alias.pass_args = true;
    }
    validate_alias(&alias)?;
    preflight_target_shells(
        config_dir,
        &alias.shells,
        args.allow_missing_loader,
        args.no_sync,
    )?;
    let mut affected = previous_shells.clone();
    for shell in &alias.shells {
        if !affected.contains(shell) {
            affected.push(shell.clone());
        }
    }
    for shell in previous_shells
        .iter()
        .filter(|shell| !alias.shells.contains(shell))
    {
        database.retire_name(&alias.name, shell, alias.revision.saturating_add(1))?;
    }
    alias.record_checksum = record_checksum(&alias)?;
    database.update_alias(&alias)?;
    sync_after_write(config_dir, &affected, args.no_sync)?;
    database
        .get_alias(alias.id)?
        .ok_or_else(|| AliasError::NotFound(args.name.clone()))
}
#[allow(dead_code)]
pub fn rename(config_dir: Option<&str>, old: &str, new: &str) -> Result<bool, AliasError> {
    rename_with_sync(config_dir, old, new, false)
}
pub fn rename_with_sync(
    config_dir: Option<&str>,
    old: &str,
    new: &str,
    no_sync: bool,
) -> Result<bool, AliasError> {
    let database = database(config_dir)?;
    let mut alias = database
        .get_alias_by_name(old)?
        .ok_or_else(|| AliasError::NotFound(old.into()))?;
    let shells = alias.shells.clone();
    for shell in &alias.shells {
        database.retire_name(old, shell, alias.revision.saturating_add(1))?;
    }
    alias.name = new.into();
    validate_alias(&alias)?;
    alias.record_checksum = record_checksum(&alias)?;
    let changed = database.update_alias(&alias)?;
    if changed {
        sync_after_write(config_dir, &shells, no_sync)?;
    }
    Ok(changed)
}
#[allow(dead_code)]
pub fn find(
    config_dir: Option<&str>,
    query: &str,
    fuzzy: bool,
    limit: Option<usize>,
    tags: Vec<String>,
) -> Result<Vec<AliasRecord>, AliasError> {
    find_query(config_dir, query, fuzzy, None, limit, tags)
}
#[allow(dead_code)]
pub fn retired_names(config_dir: Option<&str>, shell: &str) -> Result<Vec<String>, AliasError> {
    let kind = match shell {
        "zsh" => ShellKind::Zsh,
        "powershell5" => ShellKind::PowerShell5,
        "powershell7" => ShellKind::PowerShell7,
        _ => ShellKind::Bash,
    };
    Ok(database(config_dir)?.managed_name_set(&kind)?.retired)
}
pub fn shell_detect() -> aliasmgr_core::detection::DetectionResult {
    aliasmgr_core::detection::detect(&aliasmgr_core::detection::DetectionContext {
        explicit: None,
        parent_shell: None,
        shell_env: std::env::var("SHELL").ok(),
        login_shell: None,
    })
}
pub fn shell_cli_name(kind: &ShellKind) -> &'static str {
    match kind {
        ShellKind::Bash => "bash",
        ShellKind::Zsh => "zsh",
        ShellKind::PowerShell5 => "powershell5",
        ShellKind::PowerShell7 => "powershell7",
        ShellKind::Fish => "fish",
        ShellKind::PosixSh => "sh",
    }
}
pub fn detected_shell_name() -> String {
    shell_detect()
        .selected
        .map(|kind| shell_cli_name(&kind).to_string())
        .unwrap_or_else(|| "bash".to_string())
}
pub fn reload_shell(explicit: Option<&str>) -> Result<String, AliasError> {
    match explicit {
        Some(value) => shell_kind(value)
            .map(|kind| shell_cli_name(&kind).to_string())
            .ok_or_else(|| AliasError::Config(format!("unsupported shell: {value}"))),
        None => Ok(detected_shell_name()),
    }
}
pub fn sync(config_dir: Option<&str>, dry_run: bool) -> Result<String, AliasError> {
    if dry_run {
        return Ok(String::new());
    }
    let paths = AppPaths::discover(config_dir.map(Path::new));
    // Target-shell selection and the monotonic revision live in aliasmgr-core
    // so the CLI and GUI cannot diverge.
    let run = aliasmgr_core::sync::sync_config_dir(&paths.root)?;
    let failures = run.failure_summary();
    if !failures.is_empty() {
        return Err(AliasError::PartialSync(failures.join("; ")));
    }
    Ok(format!("synced {} shell results", run.results.len()))
}
pub fn reload_print(config_dir: Option<&str>, shell: &str) -> String {
    let root = AppPaths::discover(config_dir.map(Path::new)).root;
    let kind = match shell {
        "zsh" => ShellKind::Zsh,
        "powershell5" => ShellKind::PowerShell5,
        "powershell7" => ShellKind::PowerShell7,
        _ => ShellKind::Bash,
    };
    aliasmgr_core::sync::reload_command(&root, &kind)
}

#[cfg(test)]
fn default_powershell_profile_path(shell: &str) -> Option<PathBuf> {
    aliasmgr_core::integration::default_powershell_profile_path(shell)
}

fn cli_shell_kind(shell: &str) -> Result<ShellKind, AliasError> {
    shell_kind(shell).ok_or(AliasError::ShellNotInstalled)
}

pub fn shell_install(config_dir: Option<&str>, shell: &str) -> Result<bool, AliasError> {
    let paths = aliasmgr_core::config::AppPaths::discover(config_dir.map(Path::new));
    let kind = cli_shell_kind(shell)?;
    // Install first: a failed profile write must not leave the shell marked as
    // enabled without a loader. A successful install implies enablement.
    let changed = aliasmgr_core::integration::install_loader(&paths, shell)?.changed;
    shell_setup::set_shell_enabled(&paths, &kind, true)?;
    Ok(changed)
}

pub fn shell_uninstall(config_dir: Option<&str>, shell: &str) -> Result<bool, AliasError> {
    let paths = aliasmgr_core::config::AppPaths::discover(config_dir.map(Path::new));
    let kind = cli_shell_kind(shell)?;
    // Remove the loader first: a failed removal must not leave the shell
    // disabled while its loader is still installed.
    let changed = aliasmgr_core::integration::uninstall_loader(&paths, shell)?.changed;
    shell_setup::set_shell_enabled(&paths, &kind, false)?;
    Ok(changed)
}

/// Persists `shells.enabled` for one shell. Returns whether the intent changed.
pub fn shell_enable(config_dir: Option<&str>, shell: &str) -> Result<bool, AliasError> {
    let paths = aliasmgr_core::config::AppPaths::discover(config_dir.map(Path::new));
    let kind = cli_shell_kind(shell)?;
    let was_enabled = shell_setup::enabled_shells(&paths.load_config()?).contains(&kind);
    shell_setup::set_shell_enabled(&paths, &kind, true)?;
    Ok(!was_enabled)
}

/// Removes one shell from `shells.enabled`. Returns whether the intent changed.
pub fn shell_disable(config_dir: Option<&str>, shell: &str) -> Result<bool, AliasError> {
    let paths = aliasmgr_core::config::AppPaths::discover(config_dir.map(Path::new));
    let kind = cli_shell_kind(shell)?;
    let was_enabled = shell_setup::enabled_shells(&paths.load_config()?).contains(&kind);
    shell_setup::set_shell_enabled(&paths, &kind, false)?;
    Ok(was_enabled)
}

#[allow(dead_code)]
pub fn shell_install_status(config_dir: Option<&str>, shell: &str) -> Result<bool, AliasError> {
    let paths = aliasmgr_core::config::AppPaths::discover(config_dir.map(Path::new));
    Ok(aliasmgr_core::integration::loader_status(&paths, shell)?.installed)
}

pub fn export_file(config_dir: Option<&str>, file: &str) -> Result<(), AliasError> {
    let aliases = list(config_dir)?;
    let path = Path::new(file);
    if path.extension().and_then(|value| value.to_str()) == Some("toml") {
        aliasmgr_core::transfer::export_toml(path, &aliases)
    } else {
        aliasmgr_core::transfer::export_json(path, &aliases)
    }
}

fn read_import_file(file: &str) -> Result<Vec<AliasRecord>, AliasError> {
    let path = Path::new(file);
    if path.extension().and_then(|value| value.to_str()) == Some("toml") {
        aliasmgr_core::transfer::read_toml(path)
    } else {
        aliasmgr_core::transfer::read_json(path)
    }
}

fn existing_aliases(config_dir: Option<&str>) -> Result<BTreeMap<String, AliasRecord>, AliasError> {
    let path = AppPaths::discover(config_dir.map(Path::new))
        .root
        .join("aliases.db");
    if !path.exists() {
        return Ok(BTreeMap::new());
    }
    Ok(Database::open_read_only(path)?
        .list_aliases()?
        .into_iter()
        .map(|alias| (alias.name.clone(), alias))
        .collect())
}

#[allow(dead_code)]
pub fn import_file(
    config_dir: Option<&str>,
    file: &str,
    confirm: bool,
) -> Result<ImportReport, AliasError> {
    import_file_with_sync(config_dir, file, confirm, false)
}

pub fn import_file_with_sync(
    config_dir: Option<&str>,
    file: &str,
    confirm: bool,
    no_sync: bool,
) -> Result<ImportReport, AliasError> {
    if confirm {
        import_confirm_with_sync(config_dir, file, no_sync)
    } else {
        import_preview(config_dir, file)
    }
}

pub fn import_preview(config_dir: Option<&str>, file: &str) -> Result<ImportReport, AliasError> {
    let existing = existing_aliases(config_dir)?;
    aliasmgr_core::transfer::import_records(
        read_import_file(file)?,
        &existing,
        ConflictStrategy::Ask,
    )
    .map(|report| report.into_public())
}

#[allow(dead_code)]
pub fn import_confirm(config_dir: Option<&str>, file: &str) -> Result<ImportReport, AliasError> {
    import_confirm_with_sync(config_dir, file, false)
}

pub fn import_confirm_with_sync(
    config_dir: Option<&str>,
    file: &str,
    no_sync: bool,
) -> Result<ImportReport, AliasError> {
    let existing = list(config_dir)?
        .into_iter()
        .map(|alias| (alias.name.clone(), alias))
        .collect();
    let report = aliasmgr_core::transfer::import_records(
        read_import_file(file)?,
        &existing,
        ConflictStrategy::Ask,
    )?;
    database(config_dir)?.insert_aliases_atomic(&report.accepted_records)?;
    sync_after_import(config_dir, &report.accepted_records, no_sync)?;
    Ok(report.into_public())
}

/// A confirmed import is a batch write: sync all referenced shells once,
/// honoring `runtime.auto_sync` and the CLI `--no-sync` escape. Preview never
/// reaches this function.
fn sync_after_import(
    config_dir: Option<&str>,
    accepted: &[AliasRecord],
    no_sync: bool,
) -> Result<(), AliasError> {
    if no_sync || accepted.is_empty() {
        return Ok(());
    }
    let paths = paths_for(config_dir);
    let config = match paths.load_config() {
        Ok(config) => config,
        Err(error) => {
            return Err(AliasError::PartialSync(format!(
                "imported records were saved, but the configuration could not be read: {error}"
            )))
        }
    };
    if !config.runtime.auto_sync {
        return Ok(());
    }
    let run = aliasmgr_core::sync::sync_config_dir(&paths.root).map_err(|error| {
        AliasError::PartialSync(format!(
            "imported records were saved, but sync failed: {error}. Run `aliasmgr sync` or `aliasmgr doctor`."
        ))
    })?;
    let failures = run.failure_summary();
    if failures.is_empty() {
        Ok(())
    } else {
        Err(AliasError::PartialSync(format!(
            "imported records were saved, but these shells failed to sync: {}. Run `aliasmgr sync` or `aliasmgr doctor`.",
            failures.join("; ")
        )))
    }
}

/// Renders the CLI summary for an import preview or confirmation. Warnings are
/// counted here and printed line by line on stderr by the binary.
pub fn import_summary(report: &ImportReport) -> String {
    format!(
        "imported={} skipped={} unsupported={} warnings={}",
        report.imported.len(),
        report.skipped.len(),
        report.unsupported.len(),
        report.warnings.len()
    )
}
pub fn uninstall(config_dir: Option<&str>, purge: bool) -> Result<(), AliasError> {
    let paths = AppPaths::discover(config_dir.map(Path::new));
    // A broken config must not prevent purge and loader cleanup.
    let config = paths.load_config().unwrap_or_default();
    // Shared resolution includes platform defaults for PowerShell and the
    // config-root RC fallbacks for Bash/Zsh, matching the install path.
    let mut loaders = aliasmgr_core::integration::all_loader_targets(&config, &paths)
        .into_iter()
        .map(|(_, target)| target.path)
        .collect::<Vec<PathBuf>>();
    loaders.retain(|path| path.exists());
    loaders.sort();
    loaders.dedup();
    // Loader blocks live in user profiles: only a changed profile is rewritten, with a backup.
    // Do this before the destructive purge so a malformed profile aborts without data loss.
    for loader in &loaders {
        let content = std::fs::read_to_string(loader)?;
        let cleaned = aliasmgr_core::shells::common::remove_loader(&content)?;
        if cleaned != content {
            aliasmgr_core::shells::common::write_profile_content_with_keep(
                loader,
                &cleaned,
                config.backups.rc_keep,
            )?;
        }
    }
    let mode = if purge {
        aliasmgr_core::uninstall::UninstallMode::PurgeAliases
    } else {
        aliasmgr_core::uninstall::UninstallMode::RetainAliases
    };
    aliasmgr_core::uninstall::uninstall(&paths.root, mode, None)?;
    Ok(())
}

fn diagnose_findings(
    config_dir: Option<&str>,
) -> Result<Vec<aliasmgr_core::doctor::DoctorFinding>, AliasError> {
    let paths = AppPaths::discover(config_dir.map(Path::new));
    let database_path_exists = paths.root.join("aliases.db").exists();
    let database = database(config_dir)?;
    let aliases = database.list_aliases()?;
    // Shared with the GUI: probes real loader blocks, uses the same target
    // shells and revision floor as sync, and distinguishes profile errors.
    aliasmgr_core::doctor::diagnose(&paths.root, database_path_exists, &database, &aliases)
}

pub fn doctor(config_dir: Option<&str>) -> Result<Vec<String>, AliasError> {
    Ok(diagnose_findings(config_dir)?
        .into_iter()
        .map(|finding| match finding.detail {
            Some(detail)
                if matches!(
                    finding.code,
                    "sync_failed"
                        | "config_invalid"
                        | "loader_unsafe"
                        | "loader_permission"
                        | "loader_io"
                        | "loader_unreadable"
                ) =>
            {
                format!("{}: {detail}", finding.message)
            }
            _ => finding.message,
        })
        .collect())
}

#[derive(serde::Serialize)]
struct DoctorFindingJson {
    shell: Option<String>,
    severity: &'static str,
    code: &'static str,
    message: String,
    action: Option<String>,
    detail: Option<String>,
}

/// Machine-readable Doctor report for non-interactive use. Supplying
/// `--format` never opens the interactive screen.
pub fn doctor_json(config_dir: Option<&str>) -> Result<String, AliasError> {
    let findings = diagnose_findings(config_dir)?
        .into_iter()
        .map(|finding| DoctorFindingJson {
            shell: finding.shell.map(|shell| shell.as_str().to_string()),
            severity: finding.severity,
            code: finding.code,
            message: finding.message,
            action: finding.action,
            detail: finding.detail,
        })
        .collect::<Vec<_>>();
    serde_json::to_string_pretty(&findings).map_err(AliasError::from)
}

/// The interactive screen requires both streams to be terminals, so pipes,
/// CI jobs and scripted `doctor` calls keep the text/JSON contract.
pub fn doctor_is_interactive(stdin_tty: bool, stdout_tty: bool) -> bool {
    stdin_tty && stdout_tty
}

/// Parses a comma/space-separated selection of stable shell names.
#[allow(dead_code)]
pub fn parse_shell_selection(value: &str) -> Result<Vec<ShellKind>, String> {
    let mut selected = Vec::new();
    for token in value
        .split(|character: char| character == ',' || character.is_whitespace())
        .filter(|token| !token.is_empty())
    {
        let kind = ShellKind::parse_supported(token).ok_or_else(|| {
            format!("unknown shell: {token} (expected bash, zsh, powershell5, or powershell7)")
        })?;
        if !selected.contains(&kind) {
            selected.push(kind);
        }
    }
    Ok(selected)
}

#[allow(dead_code)]
fn prompt_line(
    input: &mut dyn BufRead,
    output: &mut dyn Write,
    label: &str,
) -> Result<Option<String>, AliasError> {
    write!(output, "{label}")?;
    output.flush()?;
    let mut line = String::new();
    if input.read_line(&mut line)? == 0 {
        return Ok(None);
    }
    Ok(Some(line.trim().to_string()))
}

#[allow(dead_code)]
fn prompt_path(
    input: &mut dyn BufRead,
    output: &mut dyn Write,
    label: &str,
    default: Option<&Path>,
) -> Result<Option<PathBuf>, AliasError> {
    let label = match default {
        Some(default) => format!("{label} [{}]: ", default.display()),
        None => format!("{label}: "),
    };
    let value = match prompt_line(input, output, &label)? {
        None => return Ok(None),
        Some(value) if value.is_empty() => default.map(Path::to_path_buf),
        Some(value) => Some(PathBuf::from(value)),
    };
    let Some(value) = value else {
        return Ok(None);
    };
    let expanded = aliasmgr_core::config::expand_home_path(&value);
    if !expanded.is_absolute() {
        writeln!(
            output,
            "Path must be absolute; use ~/... to expand the home directory (got {}).",
            value.display()
        )?;
        return Ok(None);
    }
    Ok(Some(expanded))
}

#[allow(dead_code)]
fn describe_installation(installation: &detection::ShellInstallation) -> String {
    if installation.wsl {
        return "WSL launcher (integrate inside the WSL distribution)".into();
    }
    if !installation.detected {
        return "not detected".into();
    }
    let path = installation
        .executable
        .as_ref()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "<unknown>".into());
    match &installation.version {
        Some(version) => format!("{path} (version {version})"),
        None => format!("{path} (version unknown)"),
    }
}

/// Line-based fallback used by `--no-default-features` builds. It still
/// completes the full flow (detect, path entry, preview, apply, sync) without
/// crossterm or raw mode.
#[allow(dead_code)]
pub fn doctor_interactive_line(
    paths: &AppPaths,
    input: &mut dyn BufRead,
    output: &mut dyn Write,
) -> Result<String, AliasError> {
    let mut summary = Vec::new();
    loop {
        writeln!(
            output,
            "Detecting shells... (version probes may take a moment)"
        )?;
        output.flush()?;
        let config = paths.load_config()?;
        writeln!(output, "Shell integration:")?;
        for kind in SUPPORTED_SHELLS {
            let installation = detection::detect_shell(&config, &kind);
            writeln!(
                output,
                "  {}  {}",
                installation.kind.label(),
                describe_installation(&installation)
            )?;
            output.flush()?;
        }
        let selection = match prompt_line(
            input,
            output,
            "Enable shells (bash,zsh,powershell5,powershell7), r=rescan, q=quit: ",
        )? {
            Some(selection) => selection,
            None => break,
        };
        if selection.eq_ignore_ascii_case("q") {
            break;
        }
        if selection.eq_ignore_ascii_case("r") {
            continue;
        }
        let shells = match parse_shell_selection(&selection) {
            Ok(shells) => shells,
            Err(error) => {
                writeln!(output, "{error}")?;
                continue;
            }
        };
        if shells.is_empty() {
            break;
        }
        for kind in shells {
            let mut plan = shell_setup::plan_shell_setup(&config, paths, &kind, None, None)?;
            let mut executable_override = None;
            let mut profile_override = None;
            let mut needs_path = false;
            match plan.action {
                shell_setup::SetupAction::WslManual => {
                    writeln!(
                        output,
                        "{} resolves to the WSL launcher. Alias Manager will not write a Windows profile for it, but you can enter a native Windows executable (for example Git Bash) and its profile/RC path.",
                        kind.label()
                    )?;
                    needs_path = true;
                }
                shell_setup::SetupAction::NeedsPath => needs_path = true,
                shell_setup::SetupAction::Enable | shell_setup::SetupAction::InstallLoader => {}
            }
            if needs_path {
                writeln!(output, "{} is not ready:", kind.label())?;
                if plan.wsl {
                    writeln!(
                        output,
                        "  1) integrate inside the WSL distribution (run `aliasmgr doctor` there), then restart"
                    )?;
                } else {
                    writeln!(
                        output,
                        "  1) install it and add it to PATH, then restart aliasmgr"
                    )?;
                }
                writeln!(
                    output,
                    "  2) enter an absolute executable and profile/RC path (leading ~/ expands)"
                )?;
                writeln!(output, "  3) cancel")?;
                let choice = prompt_line(input, output, "Choose [1-3]: ")?.unwrap_or_default();
                match choice.as_str() {
                    "1" => {
                        if plan.wsl {
                            writeln!(
                                output,
                                "Run `aliasmgr doctor` inside the WSL distribution to integrate {}.",
                                kind.label()
                            )?;
                        } else {
                            writeln!(
                                output,
                                "Install {} and add it to PATH, then run `aliasmgr doctor` again.",
                                kind.label()
                            )?;
                        }
                        continue;
                    }
                    "2" => {
                        let default_executable = if plan.wsl {
                            None
                        } else {
                            plan.executable.clone().filter(|path| path.is_file())
                        };
                        let Some(executable) = prompt_path(
                            input,
                            output,
                            "Executable path",
                            default_executable.as_deref(),
                        )?
                        else {
                            continue;
                        };
                        if !executable.is_file() {
                            writeln!(
                                output,
                                "{} executable not found: {}",
                                kind.label(),
                                executable.display()
                            )?;
                            continue;
                        }
                        let default_profile =
                            shell_setup::default_profile_path(&config, paths, &kind);
                        let profile = prompt_path(
                            input,
                            output,
                            "Profile/RC path",
                            default_profile.as_deref(),
                        )?;
                        executable_override = Some(executable);
                        profile_override = profile;
                        plan = shell_setup::plan_shell_setup(
                            &config,
                            paths,
                            &kind,
                            executable_override.as_deref(),
                            profile_override.as_deref(),
                        )?;
                        if !plan.detected {
                            writeln!(
                                output,
                                "{} executable was not detected at the supplied path.",
                                kind.label()
                            )?;
                            continue;
                        }
                    }
                    _ => continue,
                }
            }

            if let Some(preview) = shell_setup::preview_loader(
                &config,
                paths,
                &kind,
                executable_override.as_deref(),
                profile_override.as_deref(),
            )? {
                writeln!(output, "Target: {}", preview.target.display())?;
                writeln!(output, "{}", preview.block.trim_end())?;
            } else {
                writeln!(
                    output,
                    "{} has no profile path on this platform; choose option 2 and provide one.",
                    kind.label()
                )?;
                continue;
            }
            let answer =
                prompt_line(input, output, "Apply this setup? [y/N]: ")?.unwrap_or_default();
            if !matches!(answer.to_ascii_lowercase().as_str(), "y" | "yes") {
                continue;
            }
            let options = shell_setup::ShellSetupOptions {
                executable: executable_override,
                profile: profile_override,
                sync: true,
            };
            match shell_setup::apply_shell_setup(paths, &kind, &options) {
                Ok(outcome) => {
                    let mut line = format!("{}: enabled", kind.label());
                    if outcome.loader_changed {
                        line.push_str(", loader installed");
                    } else if outcome.loader_installed {
                        line.push_str(", loader already present");
                    }
                    if let Some(error) = &outcome.loader_error {
                        line.push_str(&format!(", loader error: {error}"));
                    }
                    if let Some(error) = &outcome.sync_error {
                        line.push_str(&format!(", sync error: {error}"));
                    } else if let Some(run) = &outcome.sync {
                        if run.failed() {
                            line.push_str(&format!(
                                ", sync failed: {}",
                                run.failure_summary().join("; ")
                            ));
                        } else {
                            line.push_str(", sync ok");
                        }
                    }
                    writeln!(output, "{line}")?;
                    summary.push(line);
                    if outcome.loader_installed {
                        summary.push(format!(
                            "Reload in a new shell, or run: {}",
                            aliasmgr_core::sync::reload_command(&paths.root, &kind)
                        ));
                    }
                }
                Err(error) => {
                    writeln!(output, "{}: {error}", kind.label())?;
                    summary.push(format!("{}: {error}", kind.label()));
                }
            }
        }
        break;
    }
    if summary.is_empty() {
        Ok("doctor: no changes".into())
    } else {
        Ok(summary.join("\n"))
    }
}

#[cfg(feature = "tui")]
pub fn doctor_interactive(config_dir: Option<&str>) -> Result<String, AliasError> {
    aliasmgr_tui::run_shell_setup(config_dir.map(PathBuf::from))
}

#[cfg(not(feature = "tui"))]
pub fn doctor_interactive(config_dir: Option<&str>) -> Result<String, AliasError> {
    let paths = paths_for(config_dir);
    let stdin = std::io::stdin();
    let mut input = stdin.lock();
    let stdout = std::io::stdout();
    let mut output = stdout.lock();
    doctor_interactive_line(&paths, &mut input, &mut output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    // Environment variables are process-global; these tests run in the library
    // test binary, so serialize and always restore them.
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn restore_env(saved: &[(&str, Option<OsString>)]) {
        for (key, value) in saved {
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
    }

    fn unique_home(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "aliasmgr-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or_default()
        ))
    }

    #[cfg(windows)]
    #[test]
    fn resolves_windows_powershell_profile_defaults_from_userprofile() {
        let _guard = ENV_LOCK.lock().unwrap();
        let saved = [("USERPROFILE", std::env::var_os("USERPROFILE"))];
        let home = unique_home("profile-defaults-win");
        std::env::set_var("USERPROFILE", &home);

        assert_eq!(
            default_powershell_profile_path("powershell5"),
            Some(
                home.join("Documents")
                    .join("WindowsPowerShell")
                    .join("Microsoft.PowerShell_profile.ps1")
            )
        );
        // Resolution is path-only: it does not require Documents to exist.
        std::fs::create_dir_all(home.join("Documents")).unwrap();
        assert_eq!(
            default_powershell_profile_path("powershell7"),
            Some(
                home.join("Documents")
                    .join("PowerShell")
                    .join("Microsoft.PowerShell_profile.ps1")
            )
        );
        assert_eq!(default_powershell_profile_path("bash"), None);

        restore_env(&saved);
        let _ = std::fs::remove_dir_all(home);
    }

    #[cfg(unix)]
    #[test]
    fn resolves_unix_powershell_profile_defaults_from_home() {
        let _guard = ENV_LOCK.lock().unwrap();
        let saved = [("HOME", std::env::var_os("HOME"))];
        let home = unique_home("profile-defaults-unix");
        std::env::set_var("HOME", &home);

        assert_eq!(
            default_powershell_profile_path("powershell7"),
            Some(home.join(".config").join("powershell").join("profile.ps1"))
        );
        assert_eq!(default_powershell_profile_path("powershell5"), None);

        restore_env(&saved);
    }

    #[test]
    fn dry_run_and_reload_are_non_mutating_helpers() {
        assert!(reload_print(Some("cfg"), "bash").contains("bash.sh"));
    }
    #[test]
    fn reload_shell_honours_explicit_choice_and_detection_stays_within_known_names() {
        assert_eq!(reload_shell(Some("zsh")).unwrap(), "zsh");
        assert_eq!(reload_shell(Some("ZSH")).unwrap(), "zsh");
        assert!(
            matches!(reload_shell(Some("fish")), Err(AliasError::Config(message)) if message.contains("unsupported shell: fish"))
        );
        assert!(["bash", "zsh", "powershell5", "powershell7"]
            .contains(&reload_shell(None).unwrap().as_str()));
        assert!(reload_print(Some("cfg"), "zsh").contains("zsh.sh"));
        assert!(reload_print(Some("cfg"), "powershell5").contains("powershell5.ps1"));
        assert!(reload_print(Some("cfg"), "powershell7").contains("powershell7.ps1"));
        assert!(reload_print(Some("cfg"), "bash").starts_with("source "));
        assert!(reload_print(Some("cfg"), "powershell7").starts_with(". '"));
    }

    #[test]
    fn ensure_interactive_requires_confirmation_only_without_yes() {
        let remove = Command::Remove {
            name: "gs".into(),
            yes: false,
            no_sync: false,
        };
        assert!(matches!(
            ensure_interactive(&remove, false),
            Err(AliasError::ConfirmationRequired)
        ));
        assert!(ensure_interactive(&remove, true).is_ok());
        assert!(ensure_interactive(
            &Command::Remove {
                name: "gs".into(),
                yes: true,
                no_sync: false,
            },
            false
        )
        .is_ok());

        let purge = Command::Uninstall {
            purge_aliases: true,
            yes: false,
        };
        assert!(requires_confirmation(&purge));
        assert!(matches!(
            ensure_interactive(&purge, false),
            Err(AliasError::ConfirmationRequired)
        ));
        assert!(ensure_interactive(
            &Command::Uninstall {
                purge_aliases: true,
                yes: true
            },
            false
        )
        .is_ok());
        assert!(ensure_interactive(
            &Command::Uninstall {
                purge_aliases: false,
                yes: false
            },
            false
        )
        .is_ok());
    }
}
