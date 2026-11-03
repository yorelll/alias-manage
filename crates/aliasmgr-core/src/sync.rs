use crate::{
    config::{AppConfig, AppPaths},
    error::AliasError,
    lock::FileLock,
    model::{AliasRecord, ManagedNameSet, ShellKind, ShellStatus, ShellSyncResult, SyncReceipt},
    shells::{bash, powershell, zsh},
};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

/// Normalizes CRLF to LF so a checksum is stable regardless of the line
/// ending selected for the generated file.
fn normalize_line_endings(content: &str) -> String {
    content.replace("\r\n", "\n")
}
pub fn content_checksum(content: &str) -> String {
    let content = normalize_line_endings(strip_bom(content));
    let mut digest = Sha256::new();
    digest.update(
        content
            .lines()
            .filter(|line| !line.starts_with("# file_checksum:"))
            .collect::<Vec<_>>()
            .join("\n")
            .as_bytes(),
    );
    format!("{:x}", digest.finalize())
}
pub fn verify_file_checksum(content: &str, expected: &str) -> bool {
    content_checksum(content) == expected
}
fn metadata_line(name: &str, values: &[String]) -> String {
    format!("# {name}: {}\n", values.join(" "))
}
fn backup_path(path: &Path, root: &Path) -> PathBuf {
    root.join("backups/generated")
        .join(path.file_name().unwrap())
}
fn strip_bom(content: &str) -> &str {
    content.strip_prefix('\u{FEFF}').unwrap_or(content)
}
fn configured_line_ending(value: Option<&str>) -> &'static str {
    match value.map(str::to_ascii_lowercase).as_deref() {
        Some("crlf") => "\r\n",
        _ => "\n",
    }
}
fn shell_file_content(shell: &ShellKind, body: &str, checksum: &str, config: &AppConfig) -> String {
    let (bom, line_ending) = match shell {
        ShellKind::PowerShell5 => (
            config.runtime.powershell5_bom,
            configured_line_ending(config.runtime.powershell5_line_ending.as_deref()),
        ),
        ShellKind::PowerShell7 => (
            config.runtime.powershell7_bom,
            configured_line_ending(config.runtime.powershell7_line_ending.as_deref()),
        ),
        _ => (false, "\n"),
    };
    let prefix = if bom { "\u{FEFF}" } else { "" };
    let content = format!("{body}# file_checksum: {checksum}\n");
    let content = normalize_line_endings(&content).replace('\n', line_ending);
    format!("{prefix}{content}")
}
/// Free-function form of [`SyncCoordinator::journal_needs_recovery`] for
/// front ends that only have a path.
pub fn journal_needs_recovery(path: &Path) -> Result<bool, AliasError> {
    if !path.exists() {
        return Ok(false);
    }
    let content = fs::read_to_string(path)?;
    let state = content
        .lines()
        .find_map(|line| line.strip_prefix("state="))
        .ok_or_else(|| AliasError::Config("operation journal has no state".into()))?;
    match state {
        "prepared" | "committed" => Ok(true),
        "completed" => Ok(false),
        other => Err(AliasError::Config(format!(
            "unknown operation journal state: {other}"
        ))),
    }
}

pub struct SyncCoordinator {
    pub config_dir: PathBuf,
}
pub fn shell_status(
    applied_revision: i64,
    current_revision: i64,
    loader_installed: bool,
    error: Option<&str>,
) -> ShellStatus {
    if error.is_some() {
        ShellStatus::Failed
    } else if !loader_installed {
        ShellStatus::LoaderMissing
    } else if applied_revision == current_revision {
        ShellStatus::Ok
    } else if applied_revision < current_revision {
        ShellStatus::Stale
    } else {
        ShellStatus::Unknown
    }
}

impl SyncCoordinator {
    pub fn new(config_dir: impl Into<PathBuf>) -> Self {
        Self {
            config_dir: config_dir.into(),
        }
    }
    pub fn apply_with_managed_names(
        &self,
        aliases: &[AliasRecord],
        shells: &[ShellKind],
        revision: i64,
        names: &ManagedNameSet,
    ) -> Result<SyncReceipt, AliasError> {
        self.apply_internal(aliases, shells, revision, names)
    }
    pub fn apply(
        &self,
        aliases: &[AliasRecord],
        shells: &[ShellKind],
        revision: i64,
    ) -> Result<SyncReceipt, AliasError> {
        let names = ManagedNameSet {
            current: aliases.iter().map(|alias| alias.name.clone()).collect(),
            retired: Vec::new(),
        };
        self.apply_internal(aliases, shells, revision, &names)
    }
    fn apply_internal(
        &self,
        aliases: &[AliasRecord],
        shells: &[ShellKind],
        revision: i64,
        names: &ManagedNameSet,
    ) -> Result<SyncReceipt, AliasError> {
        fs::create_dir_all(self.config_dir.join("generated"))?;
        fs::create_dir_all(self.config_dir.join("backups/generated"))?;
        let _lock = FileLock::acquire(self.config_dir.join("sync.lock"), Duration::from_secs(10))?;
        let config = AppPaths {
            root: self.config_dir.clone(),
        }
        .load_config()?;
        let database = crate::storage::Database::open(self.config_dir.join("aliases.db"))?;
        let journal = self.config_dir.join("operation.journal");
        let revision_from = revision.saturating_sub(1);
        let mut backups = Vec::new();
        for shell in shells {
            let path = self.generated_path(shell);
            if path.exists() {
                let backup = backup_path(&path, &self.config_dir);
                fs::copy(&path, &backup)?;
                backups.push(serde_json::json!({"target": path, "backup": backup}));
            }
        }
        fs::write(&journal, format!("revision_from={revision_from}\nrevision_to={revision}\nstate=prepared\nbackups_json={}\n", serde_json::to_string(&backups)?))?;
        let mut results = Vec::new();
        for shell in shells {
            let path = self.generated_path(shell);
            let temp = path.with_extension(format!("tmp.{}", std::process::id()));
            // Persist the real profile state rather than assuming a loader
            // exists. Doctor reads this column; a hardcoded `true` makes
            // `loader_missing` unreachable.
            let loader_installed = crate::integration::loader_status(
                &AppPaths {
                    root: self.config_dir.clone(),
                },
                shell.as_str(),
            )
            .map(|outcome| outcome.installed)
            .unwrap_or(false);
            let rendered = self
                .render_shell(aliases, shell, revision, names)
                .map(|body| {
                    let checksum = content_checksum(&body);
                    let file_content = shell_file_content(shell, &body, &checksum, &config);
                    (checksum, file_content)
                })
                .and_then(|(checksum, file_content)| {
                    crate::syntax::check(shell, &file_content).map(|_| (checksum, file_content))
                });
            match rendered {
                Ok((checksum, file_content)) => {
                    fs::write(&temp, &file_content)?;
                    fs::rename(&temp, &path)?;
                    let state = crate::model::ShellState {
                        shell: shell.clone(),
                        applied_revision: revision,
                        file_checksum: checksum.clone(),
                        loader_installed,
                        status: ShellStatus::Ok,
                        last_error: None,
                    };
                    database.upsert_shell_state(&state)?;
                    results.push(ShellSyncResult {
                        shell: shell.clone(),
                        status: ShellStatus::Ok,
                        error: None,
                    });
                }
                Err(error) => {
                    let message = error.to_string();
                    let _ = fs::remove_file(&temp);
                    let state = crate::model::ShellState {
                        shell: shell.clone(),
                        applied_revision: revision.saturating_sub(1),
                        file_checksum: String::new(),
                        loader_installed,
                        status: ShellStatus::Failed,
                        last_error: Some(message.clone()),
                    };
                    database.upsert_shell_state(&state)?;
                    results.push(ShellSyncResult {
                        shell: shell.clone(),
                        status: ShellStatus::Failed,
                        error: Some(message),
                    });
                }
            }
        }
        fs::write(&journal, format!("revision_from={revision_from}\nrevision_to={revision}\nstate=completed\nbackups_json={}\n", serde_json::to_string(&backups)?))?;
        Ok(SyncReceipt { revision, results })
    }
    /// Reports whether an operation journal represents interrupted work.
    ///
    /// A successful sync leaves `state=completed` behind, so the mere presence
    /// of the file is not a recovery signal. Read errors and unknown or missing
    /// states are surfaced instead of being silently treated as "nothing to do".
    pub fn journal_needs_recovery(&self) -> Result<bool, AliasError> {
        journal_needs_recovery(&self.config_dir.join("operation.journal"))
    }

    pub fn recover_pending_operations(&self) -> Result<(), AliasError> {
        let journal = self.config_dir.join("operation.journal");
        if !journal.exists() {
            return Ok(());
        }
        let content = fs::read_to_string(&journal)?;
        let config = AppPaths {
            root: self.config_dir.clone(),
        }
        .load_config()?;
        if let Some(backups) = content
            .lines()
            .find_map(|line| line.strip_prefix("backups_json="))
        {
            for entry in serde_json::from_str::<Vec<serde_json::Value>>(backups)? {
                let target = entry
                    .get("target")
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| AliasError::Config("invalid journal target".into()))?;
                if content.lines().any(|line| line == "state=prepared") {
                    let backup = entry
                        .get("backup")
                        .and_then(serde_json::Value::as_str)
                        .ok_or_else(|| AliasError::Config("invalid journal backup".into()))?;
                    if Path::new(backup).exists() {
                        fs::copy(backup, target)?;
                    }
                } else if content.lines().any(|line| line == "state=committed")
                    && !Path::new(target).exists()
                {
                    let database =
                        crate::storage::Database::open(self.config_dir.join("aliases.db"))?;
                    let aliases = database.list_aliases()?;
                    let revision = content
                        .lines()
                        .find_map(|line| line.strip_prefix("revision_to="))
                        .and_then(|value| value.parse().ok())
                        .unwrap_or(0);
                    let shell = match Path::new(target).file_name().and_then(|name| name.to_str()) {
                        Some("bash.sh") => ShellKind::Bash,
                        Some("zsh.sh") => ShellKind::Zsh,
                        Some("powershell5.ps1") => ShellKind::PowerShell5,
                        Some("powershell7.ps1") => ShellKind::PowerShell7,
                        _ => continue,
                    };
                    let names = database.managed_name_set(&shell)?;
                    // This re-render path intentionally skips syntax::check; it only serves a
                    // committed journal entry and is currently unreachable from apply_internal.
                    let body = self.render_shell(&aliases, &shell, revision, &names)?;
                    let checksum = content_checksum(&body);
                    fs::write(
                        target,
                        shell_file_content(&shell, &body, &checksum, &config),
                    )?;
                }
            }
        }
        if content.lines().any(|line| line == "state=prepared")
            || content.lines().any(|line| line == "state=committed")
        {
            fs::remove_file(journal)?;
        }
        Ok(())
    }
    fn generated_path(&self, shell: &ShellKind) -> PathBuf {
        let name = match shell {
            ShellKind::Bash => "bash.sh",
            ShellKind::Zsh => "zsh.sh",
            ShellKind::PowerShell5 => "powershell5.ps1",
            ShellKind::PowerShell7 => "powershell7.ps1",
            _ => "unsupported.sh",
        };
        self.config_dir.join("generated").join(name)
    }
    /// Renders the generated body for `shell` without touching the filesystem.
    /// Exposed so read-only diagnostics (Doctor) can compare a generated file
    /// against what the current database would produce.
    pub fn render_shell(
        &self,
        aliases: &[AliasRecord],
        shell: &ShellKind,
        revision: i64,
        names: &ManagedNameSet,
    ) -> Result<String, AliasError> {
        let mut output = format!("# Alias Manager\n# revision: {revision}\n");
        output.push_str(&metadata_line("managed", &names.current));
        output.push_str(&metadata_line("retired", &names.retired));
        for name in crate::shells::common::managed_names(names) {
            output.push_str(&format!(
                "# fingerprint: {} {}\n",
                name,
                crate::shells::common::fingerprint_marker(&name)
            ));
        }
        output.push_str(&match shell {
            ShellKind::Bash | ShellKind::Zsh => crate::shells::common::render_cleanup(names),
            ShellKind::PowerShell5 | ShellKind::PowerShell7 => powershell::render_cleanup(names),
            _ => return Err(AliasError::ShellNotInstalled),
        });
        // The canonicalization helper is shared by every restriction guard in
        // the file, so emit it once rather than per alias.
        if matches!(shell, ShellKind::PowerShell5 | ShellKind::PowerShell7)
            && aliases
                .iter()
                .any(|alias| alias.enabled && alias.restriction.is_some())
        {
            output.push_str(powershell::render_restriction_support());
        }
        for alias in aliases.iter().filter(|alias| alias.enabled) {
            output.push_str(&match shell {
                ShellKind::Bash => bash::render(alias, names)?,
                ShellKind::Zsh => zsh::render(alias, names)?,
                ShellKind::PowerShell5 | ShellKind::PowerShell7 => {
                    powershell::render_definition(alias, shell.clone(), names)?
                }
                _ => return Err(AliasError::ShellNotInstalled),
            });
        }
        Ok(output)
    }
}

/// Shells the database would render on the next sync: every shell referenced by
/// an alias, plus every shell that still holds tombstoned names.
///
/// Shared by the CLI and the GUI so both compute the same target set.
pub fn referenced_shells(
    database: &crate::storage::Database,
    aliases: &[AliasRecord],
) -> Result<Vec<ShellKind>, AliasError> {
    let mut shells = aliases
        .iter()
        .flat_map(|alias| alias.shells.clone())
        .collect::<Vec<_>>();
    for shell in [
        ShellKind::Bash,
        ShellKind::Zsh,
        ShellKind::PowerShell5,
        ShellKind::PowerShell7,
    ] {
        if !database.managed_name_set(&shell)?.retired.is_empty() && !shells.contains(&shell) {
            shells.push(shell);
        }
    }
    shells.sort();
    shells.dedup();
    Ok(shells)
}

/// Monotonic revision for the next sync. Deletes advance tombstones too, so the
/// revision never moves backwards.
pub fn current_revision(
    database: &crate::storage::Database,
    aliases: &[AliasRecord],
) -> Result<i64, AliasError> {
    Ok(aliases
        .iter()
        .map(|alias| alias.revision)
        .max()
        .unwrap_or(1)
        .max(database.max_retired_revision()?)
        .max(1))
}

/// Whether a generated file currently matches what the database would render.
///
/// Revision counters alone are not sufficient: a checked update or a delete
/// can leave the global derived revision unchanged while the expected content
/// changes. This status is computed from the actual file, like Doctor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellContentState {
    UpToDate,
    Missing,
    Unreadable,
    RenderFailed,
    Modified,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellContentStatus {
    pub shell: ShellKind,
    pub state: ShellContentState,
    pub detail: Option<String>,
}

impl ShellContentStatus {
    pub fn is_up_to_date(&self) -> bool {
        self.state == ShellContentState::UpToDate
    }
}

/// Renders the expected body for one shell and compares it with the on-disk
/// file. The comparison covers both the embedded checksum marker and the body,
/// so a hand edit is detected even when the revision metadata is unchanged.
pub fn shell_content_status(
    config_dir: &Path,
    database: &crate::storage::Database,
    aliases: &[AliasRecord],
    shell: &ShellKind,
    revision: i64,
) -> Result<ShellContentStatus, AliasError> {
    let path = AppPaths {
        root: config_dir.to_path_buf(),
    }
    .generated_path(shell.as_str());
    let status = |state, detail: Option<String>| ShellContentStatus {
        shell: shell.clone(),
        state,
        detail,
    };
    if !path.exists() {
        return Ok(status(
            ShellContentState::Missing,
            Some(path.display().to_string()),
        ));
    }
    let content = match fs::read_to_string(&path) {
        Ok(content) => content,
        Err(error) => {
            return Ok(status(
                ShellContentState::Unreadable,
                Some(error.to_string()),
            ))
        }
    };
    let names = database.managed_name_set(shell)?;
    let shell_aliases = aliases
        .iter()
        .filter(|alias| alias.shells.contains(shell))
        .cloned()
        .collect::<Vec<_>>();
    let body = match SyncCoordinator::new(config_dir).render_shell(
        &shell_aliases,
        shell,
        revision,
        &names,
    ) {
        Ok(body) => body,
        Err(error) => {
            return Ok(status(
                ShellContentState::RenderFailed,
                Some(error.to_string()),
            ))
        }
    };
    let expected = content_checksum(&body);
    let embedded = content
        .lines()
        .find_map(|line| line.strip_prefix("# file_checksum: "));
    let marker_ok = embedded == Some(expected.as_str());
    let body_ok = verify_file_checksum(&content, &expected);
    if marker_ok && body_ok {
        Ok(status(ShellContentState::UpToDate, None))
    } else {
        Ok(status(
            ShellContentState::Modified,
            Some(format!(
                "expected checksum {expected}; marker {}",
                embedded.unwrap_or("<missing>")
            )),
        ))
    }
}

/// Per-shell result of a full sync, including the path that was written,
/// whether the shell's loader block is actually installed, and a stable error
/// code for front ends that translate messages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellSyncOutcome {
    pub shell: ShellKind,
    pub status: ShellStatus,
    /// Raw error detail (used by CLI output and diagnostics).
    pub error: Option<String>,
    /// Stable code such as `sync_permission`, `sync_missing`, `sync_io`, or
    /// `sync_failed`. `None` on success.
    pub error_code: Option<String>,
    pub generated_path: PathBuf,
    pub loader_installed: bool,
    /// Stable `loader_*` code when the loader could not be verified (unsafe
    /// markers, unreadable profile, unsupported shell). `None` when the probe
    /// succeeded or the shell has no loader target.
    pub loader_error_code: Option<String>,
}

/// Stable code for an I/O failure that can be attributed to one shell.
fn io_failure_code(error: &AliasError) -> Option<&'static str> {
    match error {
        AliasError::Io(io) => Some(match io.kind() {
            std::io::ErrorKind::NotFound => "sync_missing",
            std::io::ErrorKind::PermissionDenied => "sync_permission",
            _ => "sync_io",
        }),
        _ => None,
    }
}

/// Result of syncing every referenced shell in one config root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncRun {
    pub revision: i64,
    pub results: Vec<ShellSyncOutcome>,
}

impl SyncRun {
    pub fn failed(&self) -> bool {
        self.results
            .iter()
            .any(|result| result.status == ShellStatus::Failed)
    }

    pub fn failure_summary(&self) -> Vec<String> {
        self.results
            .iter()
            .filter(|result| result.status == ShellStatus::Failed)
            .map(|result| {
                format!(
                    "{:?}: {}",
                    result.shell,
                    result.error.as_deref().unwrap_or("unknown error")
                )
            })
            .collect()
    }
}

/// Dot-source / source command that reloads a generated file in the current
/// Shell session. Paths are quoted for the target Shell so spaces, quotes and
/// non-ASCII characters survive.
pub fn reload_command(config_dir: &Path, shell: &ShellKind) -> String {
    let path = AppPaths {
        root: config_dir.to_path_buf(),
    }
    .generated_path(shell.as_str());
    match shell {
        ShellKind::PowerShell5 | ShellKind::PowerShell7 => {
            // PowerShell single-quoted strings escape a quote by doubling it;
            // the leading `.` is the dot-source operator.
            let quoted = path.to_string_lossy().replace('\'', "''");
            format!(". '{quoted}'")
        }
        _ => format!(
            "source {}",
            crate::shells::common::quote_posix(&path.to_string_lossy())
        ),
    }
}

/// Syncs every referenced shell using the shared revision and target rules.
///
/// Orchestration failures (database/config/lock errors) return `Err`. A shell
/// whose generated content fails validation is reported in `results` with
/// `ShellStatus::Failed`, matching the coordinator's partial-failure model.
pub fn sync_config_dir(config_dir: &Path) -> Result<SyncRun, AliasError> {
    let database = crate::storage::Database::open(config_dir.join("aliases.db"))?;
    let aliases = database.list_aliases()?;
    sync_with_database_mode(config_dir, &database, &aliases, false)
}

/// Like [`sync_config_dir`], but collects per-shell I/O failures into the
/// result so one unwritable shell does not discard successful shells. Used by
/// the GUI; the CLI keeps the strict variant to preserve its error contract.
pub fn sync_config_dir_collect_io(config_dir: &Path) -> Result<SyncRun, AliasError> {
    let database = crate::storage::Database::open(config_dir.join("aliases.db"))?;
    let aliases = database.list_aliases()?;
    sync_with_database_mode(config_dir, &database, &aliases, true)
}

/// Syncs only the requested shells, using the same revision, names, lock,
/// journal, backup and per-shell error rules as a full sync. Used to retry the
/// failed subset of a partial run without touching successful shells.
pub fn sync_shells(config_dir: &Path, shells: &[ShellKind]) -> Result<SyncRun, AliasError> {
    let database = crate::storage::Database::open(config_dir.join("aliases.db"))?;
    let aliases = database.list_aliases()?;
    sync_with_database_shells(config_dir, &database, &aliases, true, Some(shells))
}

/// Same as [`sync_config_dir`] but reuses an already-open database and alias
/// list. Kept public so diagnostics and tests can avoid a second connection.
pub fn sync_with_database(
    config_dir: &Path,
    database: &crate::storage::Database,
    aliases: &[AliasRecord],
) -> Result<SyncRun, AliasError> {
    sync_with_database_mode(config_dir, database, aliases, false)
}

fn sync_with_database_mode(
    config_dir: &Path,
    database: &crate::storage::Database,
    aliases: &[AliasRecord],
    collect_io: bool,
) -> Result<SyncRun, AliasError> {
    sync_with_database_shells(config_dir, database, aliases, collect_io, None)
}

fn sync_with_database_shells(
    config_dir: &Path,
    database: &crate::storage::Database,
    aliases: &[AliasRecord],
    collect_io: bool,
    only: Option<&[ShellKind]>,
) -> Result<SyncRun, AliasError> {
    let referenced = referenced_shells(database, aliases)?;
    let mut shells = match only {
        Some(selected) => {
            let mut values = selected.to_vec();
            values.sort();
            values.dedup();
            values
        }
        None => referenced.clone(),
    };
    let revision = current_revision(database, aliases)?;
    let coordinator = SyncCoordinator::new(config_dir);
    let paths = AppPaths {
        root: config_dir.to_path_buf(),
    };
    // A profile shared by multiple shells can source only one generated file.
    // Mark the other shells so the Sync page explains the shared path instead
    // of pointing at an install button Doctor will not offer.
    let config = paths.load_config().ok();
    let mut shared_paths: BTreeMap<PathBuf, usize> = BTreeMap::new();
    if let Some(config) = &config {
        for shell in &referenced {
            if let Ok(target) = crate::integration::loader_target(config, &paths, shell.as_str()) {
                *shared_paths.entry(target.path).or_default() += 1;
            }
        }
    }
    let mut results = Vec::new();
    for shell in shells.drain(..) {
        let names = database.managed_name_set(&shell)?;
        let shell_aliases = aliases
            .iter()
            .filter(|alias| alias.shells.contains(&shell))
            .cloned()
            .collect::<Vec<_>>();
        let (loader_installed, mut loader_error_code) =
            match crate::integration::loader_status(&paths, shell.as_str()) {
                Ok(outcome) => (outcome.installed, None),
                Err(error) => (
                    false,
                    Some(crate::integration::loader_error_code(&error).to_string()),
                ),
            };
        if !loader_installed && loader_error_code.is_none() {
            if let Some(config) = &config {
                if let Ok(target) =
                    crate::integration::loader_target(config, &paths, shell.as_str())
                {
                    if shared_paths.get(&target.path).copied().unwrap_or(0) > 1 {
                        loader_error_code = Some("loader_shared_path".to_string());
                    }
                }
            }
        }
        let generated_path = paths.generated_path(shell.as_str());
        let receipt = match coordinator.apply_with_managed_names(
            &shell_aliases,
            std::slice::from_ref(&shell),
            revision,
            &names,
        ) {
            Ok(receipt) => receipt,
            Err(error) => {
                // Only I/O failures are safe to attribute to one shell and
                // continue past. Config, lock, and database errors affect the
                // whole run and propagate unchanged (CLI exit codes stay put).
                let Some(code) = io_failure_code(&error) else {
                    return Err(error);
                };
                if !collect_io {
                    return Err(error);
                }
                let message = error.to_string();
                let _ = database.upsert_shell_state(&crate::model::ShellState {
                    shell: shell.clone(),
                    applied_revision: revision.saturating_sub(1),
                    file_checksum: String::new(),
                    loader_installed,
                    status: ShellStatus::Failed,
                    last_error: Some(message.clone()),
                });
                results.push(ShellSyncOutcome {
                    shell,
                    status: ShellStatus::Failed,
                    error: Some(message),
                    error_code: Some(code.to_string()),
                    generated_path,
                    loader_installed,
                    loader_error_code: loader_error_code.clone(),
                });
                continue;
            }
        };
        for result in receipt.results {
            let generated_path = paths.generated_path(result.shell.as_str());
            let error_code =
                (result.status == ShellStatus::Failed).then(|| "sync_failed".to_string());
            results.push(ShellSyncOutcome {
                shell: result.shell,
                status: result.status,
                error: result.error,
                error_code,
                generated_path,
                loader_installed,
                loader_error_code: loader_error_code.clone(),
            });
        }
    }
    Ok(SyncRun { revision, results })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::TargetType;
    #[test]
    fn apply_with_managed_names_preserves_retired_tombstones_and_fingerprints() {
        let root = std::env::temp_dir().join(format!("aliasmgr-tombstone-{}", std::process::id()));
        let alias = AliasRecord {
            name: "new".into(),
            executable: "git".into(),
            ..Default::default()
        };
        let names = ManagedNameSet {
            current: vec!["new".into()],
            retired: vec!["old".into()],
        };
        SyncCoordinator::new(&root)
            .apply_with_managed_names(&[alias], &[ShellKind::Bash], 3, &names)
            .unwrap();
        let content = fs::read_to_string(root.join("generated/bash.sh")).unwrap();
        assert!(content.contains("# retired: old"));
        assert!(content.contains("aliasmgr fingerprint:"));
        let _ = fs::remove_dir_all(root);
    }
    #[test]
    fn generated_files_include_stable_checksum_metadata() {
        let root = std::env::temp_dir().join(format!("aliasmgr-checksum-{}", std::process::id()));
        let alias = AliasRecord {
            name: "gs".into(),
            executable: "git".into(),
            ..Default::default()
        };
        SyncCoordinator::new(&root)
            .apply(&[alias], &[ShellKind::Bash], 2)
            .unwrap();
        let content = fs::read_to_string(root.join("generated/bash.sh")).unwrap();
        assert!(content.contains("# revision: 2"));
        assert!(content.contains("# managed: gs"));
        let checksum = content
            .lines()
            .find_map(|line| line.strip_prefix("# file_checksum: "))
            .unwrap();
        assert!(verify_file_checksum(&content, checksum));
        let _ = fs::remove_dir_all(root);
    }
    #[test]
    fn checksum_verification_detects_manual_generated_file_edits() {
        let original = "# revision: 2\n# managed: gs\n# retired: \nfunction gs() { command git \"$@\"; }\n# file_checksum: ignored\n";
        let checksum = content_checksum(original);
        assert!(verify_file_checksum(original, &checksum));
        assert!(!verify_file_checksum(
            &original.replace("command git", "command rm"),
            &checksum
        ));
    }
    #[test]
    fn computes_shell_states() {
        assert_eq!(shell_status(2, 2, true, None), ShellStatus::Ok);
        assert_eq!(shell_status(1, 2, true, None), ShellStatus::Stale);
        assert_eq!(shell_status(2, 2, false, None), ShellStatus::LoaderMissing);
        assert_eq!(
            shell_status(1, 2, true, Some("syntax")),
            ShellStatus::Failed
        );
    }
    #[test]
    fn applies_each_shell_and_writes_journal() {
        let root = std::env::temp_dir().join(format!("aliasmgr-sync-{}", std::process::id()));
        let alias = AliasRecord {
            name: "gs".into(),
            executable: "git".into(),
            target_type: TargetType::NativeExecutable,
            ..Default::default()
        };
        let receipt = SyncCoordinator::new(&root)
            .apply(&[alias], &[ShellKind::Bash, ShellKind::Zsh], 2)
            .unwrap();
        assert_eq!(receipt.results.len(), 2);
        assert!(root.join("operation.journal").exists());
        assert!(root.join("generated/bash.sh").exists());
        let _ = fs::remove_dir_all(root);
    }
    #[test]
    fn partial_shell_failure_keeps_successful_shell_result() {
        let root = std::env::temp_dir().join(format!("aliasmgr-partial-{}", std::process::id()));
        let alias = AliasRecord {
            name: "bad".into(),
            executable: "tool".into(),
            advanced_shell_mode: true,
            ..Default::default()
        };
        let receipt = SyncCoordinator::new(&root)
            .apply(&[alias], &[ShellKind::Bash, ShellKind::PowerShell7], 2)
            .unwrap();
        assert_eq!(receipt.results.len(), 2);
        assert!(receipt
            .results
            .iter()
            .any(|result| result.status == ShellStatus::Failed));
        let _ = fs::remove_dir_all(root);
    }
    #[test]
    fn recovery_restores_generated_file_from_prepared_journal_backup() {
        let root = std::env::temp_dir().join(format!("aliasmgr-recovery-{}", std::process::id()));
        let generated = root.join("generated/bash.sh");
        let backup = root.join("backups/generated/bash.sh.bak");
        fs::create_dir_all(generated.parent().unwrap()).unwrap();
        fs::create_dir_all(backup.parent().unwrap()).unwrap();
        fs::write(&generated, "old generated content\n").unwrap();
        fs::copy(&generated, &backup).unwrap();
        fs::write(&generated, "partially replaced content\n").unwrap();
        let entries = serde_json::json!([{ "target": generated.to_string_lossy(), "backup": backup.to_string_lossy() }]);
        fs::write(
            root.join("operation.journal"),
            format!("revision_from=1\nrevision_to=2\nstate=prepared\nbackups_json={entries}\n"),
        )
        .unwrap();
        SyncCoordinator::new(&root)
            .recover_pending_operations()
            .unwrap();
        assert_eq!(
            fs::read_to_string(&generated).unwrap(),
            "old generated content\n"
        );
        assert!(!root.join("operation.journal").exists());
        let _ = fs::remove_dir_all(root);
    }
    #[test]
    fn apply_records_backup_for_existing_generated_file() {
        let root = std::env::temp_dir().join(format!("aliasmgr-backup-{}", std::process::id()));
        let generated = root.join("generated/bash.sh");
        fs::create_dir_all(generated.parent().unwrap()).unwrap();
        fs::write(&generated, "previous\n").unwrap();
        let alias = AliasRecord {
            name: "gs".into(),
            executable: "git".into(),
            ..Default::default()
        };
        SyncCoordinator::new(&root)
            .apply(&[alias], &[ShellKind::Bash], 2)
            .unwrap();
        let journal = fs::read_to_string(root.join("operation.journal")).unwrap();
        assert!(journal.contains("backups_json=["));
        assert!(fs::read_dir(root.join("backups/generated"))
            .unwrap()
            .next()
            .is_some());
        let _ = fs::remove_dir_all(root);
    }
    #[test]
    fn cleanup_clears_retired_names_even_without_current_aliases() {
        let root =
            std::env::temp_dir().join(format!("aliasmgr-cleanup-retired-{}", std::process::id()));
        let names = ManagedNameSet {
            current: vec![],
            retired: vec!["old".into()],
        };
        let receipt = SyncCoordinator::new(&root)
            .apply_with_managed_names(
                &[],
                &[ShellKind::Bash, ShellKind::Zsh, ShellKind::PowerShell5],
                4,
                &names,
            )
            .unwrap();
        assert!(receipt
            .results
            .iter()
            .all(|result| result.status == ShellStatus::Ok));
        let bash = fs::read_to_string(root.join("generated/bash.sh")).unwrap();
        assert!(bash.contains("unalias 'old'"));
        assert!(bash.contains("unset -f 'old'"));
        let zsh = fs::read_to_string(root.join("generated/zsh.sh")).unwrap();
        assert!(zsh.contains("unalias 'old'"));
        assert!(zsh.contains("unset -f 'old'"));
        let powershell = fs::read_to_string(root.join("generated/powershell5.ps1")).unwrap();
        assert!(powershell.contains("Alias:\\old"));
        let _ = fs::remove_dir_all(root);
    }
    #[test]
    fn cleanup_block_is_emitted_once_per_generated_file() {
        let root =
            std::env::temp_dir().join(format!("aliasmgr-cleanup-once-{}", std::process::id()));
        let alias = AliasRecord {
            name: "gs".into(),
            executable: "git".into(),
            ..Default::default()
        };
        SyncCoordinator::new(&root)
            .apply(&[alias], &[ShellKind::Bash], 2)
            .unwrap();
        let content = fs::read_to_string(root.join("generated/bash.sh")).unwrap();
        assert_eq!(content.matches("# aliasmgr cleanup:").count(), 1);
        let _ = fs::remove_dir_all(root);
    }
    #[test]
    fn powershell5_generated_file_is_utf8_bom_encoded_by_default() {
        let root = std::env::temp_dir().join(format!("aliasmgr-bom5-{}", std::process::id()));
        let alias = AliasRecord {
            name: "gs".into(),
            executable: "git.exe".into(),
            ..Default::default()
        };
        let coordinator = SyncCoordinator::new(&root);
        coordinator
            .apply(std::slice::from_ref(&alias), &[ShellKind::PowerShell5], 2)
            .unwrap();
        let path = root.join("generated/powershell5.ps1");
        let bytes = fs::read(&path).unwrap();
        assert!(bytes.starts_with(&[0xEF, 0xBB, 0xBF]));
        let content = fs::read_to_string(&path).unwrap();
        let checksum = content
            .lines()
            .find_map(|line| line.strip_prefix("# file_checksum: "))
            .unwrap();
        assert!(verify_file_checksum(&content, checksum));
        coordinator
            .apply(&[alias], &[ShellKind::PowerShell7], 3)
            .unwrap();
        let seven = fs::read(root.join("generated/powershell7.ps1")).unwrap();
        assert!(!seven.starts_with(&[0xEF, 0xBB, 0xBF]));
        let _ = fs::remove_dir_all(root);
    }
    fn bash_available() -> bool {
        std::process::Command::new("bash")
            .arg("--version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }
    #[test]
    fn cleanup_markers_match_the_number_of_managed_names() {
        let root =
            std::env::temp_dir().join(format!("aliasmgr-cleanup-two-{}", std::process::id()));
        let aliases = vec![
            AliasRecord {
                name: "one".into(),
                executable: "git".into(),
                ..Default::default()
            },
            AliasRecord {
                name: "two".into(),
                executable: "git".into(),
                ..Default::default()
            },
        ];
        SyncCoordinator::new(&root)
            .apply(&aliases, &[ShellKind::Bash], 2)
            .unwrap();
        let content = fs::read_to_string(root.join("generated/bash.sh")).unwrap();
        assert_eq!(content.matches("# aliasmgr cleanup:").count(), 2);
        let _ = fs::remove_dir_all(root);
    }
    #[test]
    fn syntax_failure_preserves_previous_generated_file_and_leaves_no_temp() {
        let _lock = crate::syntax::INTERPRETER_PATH_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if !bash_available() {
            return;
        }
        let root =
            std::env::temp_dir().join(format!("aliasmgr-syntax-preserve-{}", std::process::id()));
        let good = AliasRecord {
            name: "gs".into(),
            executable: "git".into(),
            ..Default::default()
        };
        SyncCoordinator::new(&root)
            .apply(&[good], &[ShellKind::Bash], 1)
            .unwrap();
        let previous = fs::read_to_string(root.join("generated/bash.sh")).unwrap();
        let broken = AliasRecord {
            name: "bad(".into(),
            executable: "git".into(),
            ..Default::default()
        };
        let receipt = SyncCoordinator::new(&root)
            .apply(&[broken], &[ShellKind::Bash], 2)
            .unwrap();
        assert_eq!(receipt.results[0].status, ShellStatus::Failed);
        assert!(receipt.results[0]
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("syntax check"));
        assert_eq!(
            fs::read_to_string(root.join("generated/bash.sh")).unwrap(),
            previous
        );
        let leftovers = fs::read_dir(root.join("generated"))
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().contains(".tmp."))
            .count();
        assert_eq!(leftovers, 0);
        let _ = fs::remove_dir_all(root);
    }
    #[test]
    fn powershell7_bom_can_be_enabled_through_config() {
        let root = std::env::temp_dir().join(format!("aliasmgr-bom7-on-{}", std::process::id()));
        let mut config = AppConfig::default();
        config.runtime.powershell7_bom = true;
        config.save(&AppPaths { root: root.clone() }).unwrap();
        let alias = AliasRecord {
            name: "gs".into(),
            executable: "git.exe".into(),
            ..Default::default()
        };
        SyncCoordinator::new(&root)
            .apply(&[alias], &[ShellKind::PowerShell7], 2)
            .unwrap();
        let bytes = fs::read(root.join("generated/powershell7.ps1")).unwrap();
        assert!(bytes.starts_with(&[0xEF, 0xBB, 0xBF]));
        let _ = fs::remove_dir_all(root);
    }
    #[test]
    fn checksum_is_stable_across_lf_and_crlf_line_endings() {
        let lf = "# revision: 1\n# managed: gs\nfunction gs() { command git \"$@\"; }\n# file_checksum: ignored\n";
        let crlf = lf.replace('\n', "\r\n");
        assert_eq!(content_checksum(lf), content_checksum(&crlf));
        assert!(verify_file_checksum(&crlf, &content_checksum(lf)));
    }
    #[test]
    fn powershell5_crlf_line_ending_config_writes_crlf_bytes() {
        let root = std::env::temp_dir().join(format!("aliasmgr-crlf5-{}", std::process::id()));
        let mut config = AppConfig::default();
        config.runtime.powershell5_line_ending = Some("crlf".into());
        config.save(&AppPaths { root: root.clone() }).unwrap();
        let alias = AliasRecord {
            name: "gs".into(),
            executable: "git.exe".into(),
            ..Default::default()
        };
        SyncCoordinator::new(&root)
            .apply(&[alias], &[ShellKind::PowerShell5], 2)
            .unwrap();
        let bytes = fs::read(root.join("generated/powershell5.ps1")).unwrap();
        assert!(
            bytes.starts_with(&[0xEF, 0xBB, 0xBF]),
            "BOM must still be written"
        );
        assert!(
            bytes.windows(2).any(|window| window == b"\r\n"),
            "expected CRLF bytes"
        );
        assert!(
            !bytes
                .windows(2)
                .any(|window| window[1] == b'\n' && window[0] != b'\r'),
            "expected no bare LF bytes"
        );
        let content = String::from_utf8(bytes).unwrap();
        let checksum = content
            .lines()
            .find_map(|line| line.strip_prefix("# file_checksum: "))
            .unwrap();
        assert!(verify_file_checksum(&content, checksum));
        let _ = fs::remove_dir_all(root);
    }
    #[test]
    fn powershell7_line_ending_defaults_to_lf_and_accepts_explicit_lf() {
        let root = std::env::temp_dir().join(format!("aliasmgr-lf7-{}", std::process::id()));
        let mut config = AppConfig::default();
        config.runtime.powershell7_line_ending = Some("lf".into());
        config.save(&AppPaths { root: root.clone() }).unwrap();
        let alias = AliasRecord {
            name: "gs".into(),
            executable: "git.exe".into(),
            ..Default::default()
        };
        SyncCoordinator::new(&root)
            .apply(&[alias], &[ShellKind::PowerShell7], 2)
            .unwrap();
        let bytes = fs::read(root.join("generated/powershell7.ps1")).unwrap();
        assert!(!bytes.contains(&b'\r'), "expected LF-only bytes");
        assert!(bytes.windows(2).any(|window| window == b"\n#"));
        let _ = std::fs::remove_dir_all(root);
    }
    #[test]
    fn powershell_line_ending_defaults_to_lf_when_unset() {
        let root = std::env::temp_dir().join(format!("aliasmgr-lf-none-{}", std::process::id()));
        let alias = AliasRecord {
            name: "gs".into(),
            executable: "git.exe".into(),
            ..Default::default()
        };
        SyncCoordinator::new(&root)
            .apply(
                &[alias],
                &[ShellKind::PowerShell5, ShellKind::PowerShell7],
                2,
            )
            .unwrap();
        let five = fs::read(root.join("generated/powershell5.ps1")).unwrap();
        assert!(five.starts_with(&[0xEF, 0xBB, 0xBF]));
        assert!(
            !five.contains(&b'\r'),
            "unset PS5 line ending must default to LF"
        );
        let seven = fs::read(root.join("generated/powershell7.ps1")).unwrap();
        assert!(
            !seven.contains(&b'\r'),
            "unset PS7 line ending must default to LF"
        );
        let _ = fs::remove_dir_all(root);
    }
    #[test]
    fn unknown_powershell_line_ending_values_fall_back_to_lf() {
        let root = std::env::temp_dir().join(format!("aliasmgr-lf-unknown-{}", std::process::id()));
        let mut config = AppConfig::default();
        // A trailing space and an unrelated value are both unsupported.
        config.runtime.powershell5_line_ending = Some("CRLF ".into());
        config.runtime.powershell7_line_ending = Some("invalid".into());
        config.save(&AppPaths { root: root.clone() }).unwrap();
        let alias = AliasRecord {
            name: "gs".into(),
            executable: "git.exe".into(),
            ..Default::default()
        };
        SyncCoordinator::new(&root)
            .apply(
                &[alias],
                &[ShellKind::PowerShell5, ShellKind::PowerShell7],
                2,
            )
            .unwrap();
        for file in ["powershell5.ps1", "powershell7.ps1"] {
            let bytes = fs::read(root.join("generated").join(file)).unwrap();
            assert!(!bytes.contains(&b'\r'), "{file} must fall back to LF");
        }
        let _ = fs::remove_dir_all(root);
    }
    #[test]
    fn powershell5_explicit_lf_line_ending_has_no_crlf_and_keeps_bom() {
        let root =
            std::env::temp_dir().join(format!("aliasmgr-lf-explicit-{}", std::process::id()));
        let mut config = AppConfig::default();
        config.runtime.powershell5_line_ending = Some("lf".into());
        config.save(&AppPaths { root: root.clone() }).unwrap();
        let alias = AliasRecord {
            name: "gs".into(),
            executable: "git.exe".into(),
            ..Default::default()
        };
        SyncCoordinator::new(&root)
            .apply(&[alias], &[ShellKind::PowerShell5], 2)
            .unwrap();
        let bytes = fs::read(root.join("generated/powershell5.ps1")).unwrap();
        assert!(bytes.starts_with(&[0xEF, 0xBB, 0xBF]));
        assert!(
            !bytes.contains(&b'\r'),
            "explicit LF must not write CR bytes"
        );
        let _ = fs::remove_dir_all(root);
    }
    #[test]
    fn powershell5_bom_can_be_disabled_through_config() {
        let root = std::env::temp_dir().join(format!("aliasmgr-bom5-off-{}", std::process::id()));
        let mut config = AppConfig::default();
        config.runtime.powershell5_bom = false;
        config.save(&AppPaths { root: root.clone() }).unwrap();
        let alias = AliasRecord {
            name: "gs".into(),
            executable: "git.exe".into(),
            ..Default::default()
        };
        SyncCoordinator::new(&root)
            .apply(&[alias], &[ShellKind::PowerShell5], 2)
            .unwrap();
        let bytes = fs::read(root.join("generated/powershell5.ps1")).unwrap();
        assert!(!bytes.starts_with(&[0xEF, 0xBB, 0xBF]));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn powershell_restriction_support_is_emitted_once_per_generated_file() {
        let names = ManagedNameSet {
            current: vec!["rma".into(), "clean".into(), "plain".into()],
            retired: vec![],
        };

        let restricted = |name: &str| AliasRecord {
            name: name.into(),
            executable: "Remove-Item".into(),
            restriction: Some(crate::model::PathRestriction {
                mode: crate::model::RestrictionMode::Whitelist,
                paths: vec!["C:\\work".into()],
            }),
            ..Default::default()
        };
        let aliases = vec![
            restricted("rma"),
            restricted("clean"),
            AliasRecord {
                name: "plain".into(),
                executable: "git.exe".into(),
                ..Default::default()
            },
        ];
        let body = SyncCoordinator::new(std::env::temp_dir())
            .render_shell(&aliases, &ShellKind::PowerShell7, 1, &names)
            .unwrap();
        assert_eq!(body.matches("# aliasmgr restriction support").count(), 1);
        assert_eq!(
            body.matches("function global:__aliasmgr_canonical_path")
                .count(),
            1
        );
        assert_eq!(body.matches("$PSCmdlet.WriteError").count(), 4, "{body}");

        let plain = SyncCoordinator::new(std::env::temp_dir())
            .render_shell(&aliases[2..], &ShellKind::PowerShell7, 1, &names)
            .unwrap();
        assert!(!plain.contains("# aliasmgr restriction support"));
    }

    fn sync_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "aliasmgr-sync-run-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or_default()
        ))
    }

    #[test]
    fn sync_config_dir_writes_a_file_for_every_referenced_shell() {
        let root = sync_root("referenced");
        // A real, existing target with spaces in its path. Creating it under
        // the sync temp root keeps this test hermetic on Linux and Windows.
        let target = root
            .join("Program Files")
            .join("LocalSend")
            .join("localsend_app.exe");
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(&target, b"stub").unwrap();
        let target_string = target.to_string_lossy().into_owned();
        let database = crate::storage::Database::open(root.join("aliases.db")).unwrap();
        database
            .insert_alias(&AliasRecord {
                name: "lsend2".into(),
                executable: target_string.clone(),
                shells: vec![ShellKind::PowerShell5, ShellKind::PowerShell7],
                revision: 3,
                ..Default::default()
            })
            .unwrap();
        drop(database);

        let run = sync_config_dir(&root).unwrap();
        assert_eq!(run.revision, 3);
        assert_eq!(run.results.len(), 2);
        assert!(!run.failed());
        for result in &run.results {
            assert_eq!(result.status, ShellStatus::Ok);
            assert!(result.generated_path.exists());
            let content = fs::read_to_string(&result.generated_path).unwrap();
            assert!(content.contains("# revision: 3"), "{content}");
            assert!(content.contains("lsend2"), "{content}");
            assert!(
                content.contains("Program Files") && content.contains("LocalSend"),
                "the spaced temp target must be rendered: {content}"
            );
            assert!(content.contains(&target_string), "{content}");
        }
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn sync_config_dir_with_no_aliases_or_shells_is_a_noop() {
        let root = sync_root("empty");
        let run = sync_config_dir(&root).unwrap();
        assert_eq!(run.revision, 1);
        assert!(run.results.is_empty());
        assert!(!run.failed());
        assert!(
            !root.join("generated").exists(),
            "an empty install must not create generated files"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn sync_config_dir_includes_shells_that_only_have_tombstones() {
        let root = sync_root("tombstones");
        let database = crate::storage::Database::open(root.join("aliases.db")).unwrap();
        database.retire_name("gone", &ShellKind::Zsh, 5).unwrap();
        drop(database);

        let run = sync_config_dir(&root).unwrap();
        assert_eq!(run.revision, 5);
        assert_eq!(run.results.len(), 1);
        assert_eq!(run.results[0].shell, ShellKind::Zsh);
        let content = fs::read_to_string(root.join("generated/zsh.sh")).unwrap();
        assert!(content.contains("# revision: 5"));
        assert!(content.contains("unalias 'gone'"), "{content}");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn sync_config_dir_reports_per_shell_failures_without_losing_successes() {
        let root = sync_root("partial");
        let database = crate::storage::Database::open(root.join("aliases.db")).unwrap();
        for (name, shells, advanced) in [
            ("good", vec![ShellKind::Bash], false),
            ("bad", vec![ShellKind::Zsh], true),
        ] {
            database
                .insert_alias(&AliasRecord {
                    name: name.into(),
                    executable: "git".into(),
                    shells,
                    advanced_shell_mode: advanced,
                    ..Default::default()
                })
                .unwrap();
        }
        drop(database);

        let run = sync_config_dir(&root).unwrap();
        assert!(run.failed());
        assert_eq!(run.results.len(), 2);
        let bash = run
            .results
            .iter()
            .find(|result| result.shell == ShellKind::Bash)
            .unwrap();
        let zsh = run
            .results
            .iter()
            .find(|result| result.shell == ShellKind::Zsh)
            .unwrap();
        assert_eq!(bash.status, ShellStatus::Ok);
        assert_eq!(zsh.status, ShellStatus::Failed);
        assert!(zsh.error.is_some());
        assert!(bash.generated_path.exists());
        assert!(!run.failure_summary().is_empty());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn repeated_sync_is_idempotent() {
        let root = sync_root("idempotent");
        let database = crate::storage::Database::open(root.join("aliases.db")).unwrap();
        database
            .insert_alias(&AliasRecord {
                name: "gs".into(),
                executable: "git".into(),
                shells: vec![ShellKind::Bash, ShellKind::PowerShell7],
                revision: 2,
                ..Default::default()
            })
            .unwrap();
        drop(database);

        let first = sync_config_dir(&root).unwrap();
        let first_files = first
            .results
            .iter()
            .map(|result| fs::read(&result.generated_path).unwrap())
            .collect::<Vec<_>>();
        let second = sync_config_dir(&root).unwrap();
        let second_files = second
            .results
            .iter()
            .map(|result| fs::read(&result.generated_path).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(first.revision, second.revision);
        assert_eq!(first_files, second_files);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn sync_revision_outranks_tombstones_after_a_delete() {
        let root = sync_root("revision");
        let database = crate::storage::Database::open(root.join("aliases.db")).unwrap();
        database
            .insert_alias(&AliasRecord {
                name: "gs".into(),
                executable: "git".into(),
                shells: vec![ShellKind::Bash],
                revision: 2,
                ..Default::default()
            })
            .unwrap();
        database
            .retire_name("deleted", &ShellKind::Bash, 9)
            .unwrap();
        drop(database);

        let run = sync_config_dir(&root).unwrap();
        assert_eq!(run.revision, 9);
        let content = fs::read_to_string(root.join("generated/bash.sh")).unwrap();
        assert!(content.contains("# revision: 9"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn sync_supports_a_config_root_with_spaces_quotes_and_cjk() {
        let root = sync_root("路径 path's dir");
        let database = crate::storage::Database::open(root.join("aliases.db")).unwrap();
        database
            .insert_alias(&AliasRecord {
                name: "unicode_path".into(),
                executable: "git".into(),
                shells: vec![ShellKind::Bash],
                ..Default::default()
            })
            .unwrap();
        drop(database);

        let run = sync_config_dir(&root).unwrap();
        assert_eq!(run.results.len(), 1);
        assert!(run.results[0].generated_path.exists());
        let command = reload_command(&root, &ShellKind::Bash);
        assert!(command.starts_with("source '"), "{command}");
        assert!(command.contains("路径 path'\\''s dir"), "{command}");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn reload_commands_are_label_safe_for_every_supported_shell() {
        let root = Path::new("/tmp/alias mgr");
        for (shell, prefix) in [
            (ShellKind::Bash, "source "),
            (ShellKind::Zsh, "source "),
            (ShellKind::PowerShell5, ". "),
            (ShellKind::PowerShell7, ". "),
        ] {
            let command = reload_command(root, &shell);
            let generated = AppPaths {
                root: root.to_path_buf(),
            }
            .generated_path(shell.as_str());
            assert!(command.starts_with(prefix), "{command}");
            assert!(
                command.contains(&generated.to_string_lossy().into_owned()),
                "{command}"
            );
        }
        let quoted = reload_command(Path::new("/tmp/it's"), &ShellKind::PowerShell7);
        assert!(quoted.contains("it''s"), "{quoted}");
        let posix = reload_command(Path::new("/tmp/it's"), &ShellKind::Bash);
        assert!(posix.contains("it'\\''s"), "{posix}");
    }

    #[test]
    fn sync_fails_cleanly_when_the_config_file_is_malformed() {
        let root = sync_root("bad-config");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("config.toml"), "not = [valid").unwrap();
        let database = crate::storage::Database::open(root.join("aliases.db")).unwrap();
        database
            .insert_alias(&AliasRecord {
                name: "gs".into(),
                executable: "git".into(),
                shells: vec![ShellKind::Bash],
                ..Default::default()
            })
            .unwrap();
        drop(database);

        assert!(matches!(sync_config_dir(&root), Err(AliasError::Config(_))));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn sync_fails_cleanly_when_the_database_is_malformed() {
        let root = sync_root("bad-db");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("aliases.db"), b"this is not a sqlite database").unwrap();
        assert!(sync_config_dir(&root).is_err());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn sync_fails_when_the_generated_directory_cannot_be_created() {
        let root = sync_root("generated-is-file");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("generated"), "not a directory").unwrap();
        let database = crate::storage::Database::open(root.join("aliases.db")).unwrap();
        database
            .insert_alias(&AliasRecord {
                name: "gs".into(),
                executable: "git".into(),
                shells: vec![ShellKind::Bash],
                ..Default::default()
            })
            .unwrap();
        drop(database);

        assert!(sync_config_dir(&root).is_err());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn a_maximum_length_alias_name_syncs() {
        let root = sync_root("long-name");
        let name = format!("a{}", "b".repeat(63));
        assert_eq!(name.len(), 64);
        let database = crate::storage::Database::open(root.join("aliases.db")).unwrap();
        database
            .insert_alias(&AliasRecord {
                name: name.clone(),
                executable: "git".into(),
                shells: vec![ShellKind::Bash],
                ..Default::default()
            })
            .unwrap();
        drop(database);

        let run = sync_config_dir(&root).unwrap();
        assert_eq!(run.results[0].status, ShellStatus::Ok);
        assert!(fs::read_to_string(root.join("generated/bash.sh"))
            .unwrap()
            .contains(&name));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn sync_creates_a_missing_config_root_instead_of_failing() {
        let root = sync_root("missing-root").join("nested/config");
        assert!(!root.exists());

        let run = sync_config_dir(&root).unwrap();
        assert!(run.results.is_empty());
        assert!(root.join("aliases.db").exists());
        let _ = fs::remove_dir_all(root.parent().unwrap().parent().unwrap());
    }

    #[test]
    fn an_alias_with_no_shells_does_not_create_a_generated_file() {
        let root = sync_root("no-shells");
        let database = crate::storage::Database::open(root.join("aliases.db")).unwrap();
        database
            .insert_alias(&AliasRecord {
                name: "orphan".into(),
                executable: "git".into(),
                shells: Vec::new(),
                ..Default::default()
            })
            .unwrap();
        drop(database);

        let run = sync_config_dir(&root).unwrap();
        assert!(run.results.is_empty());
        assert!(!root.join("generated").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(windows)]
    #[test]
    fn a_read_only_generated_file_makes_the_sync_report_an_error() {
        let root = sync_root("readonly");
        let database = crate::storage::Database::open(root.join("aliases.db")).unwrap();
        database
            .insert_alias(&AliasRecord {
                name: "gs".into(),
                executable: "git".into(),
                shells: vec![ShellKind::Bash],
                ..Default::default()
            })
            .unwrap();
        drop(database);
        sync_config_dir(&root).unwrap();

        let generated = root.join("generated/bash.sh");
        let mut permissions = fs::metadata(&generated).unwrap().permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&generated, permissions).unwrap();
        let result = sync_config_dir(&root);
        // Restore writability before cleanup so remove_dir_all can succeed.
        let mut permissions = fs::metadata(&generated).unwrap().permissions();
        #[allow(clippy::permissions_set_readonly_false)]
        permissions.set_readonly(false);
        fs::set_permissions(&generated, permissions).unwrap();

        assert!(
            result.is_err(),
            "replacing a read-only generated file must surface an error"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn sync_persists_the_real_loader_state_instead_of_hardcoding_true() {
        let root = sync_root("loader-state");
        let paths = AppPaths { root: root.clone() };
        let profile = root.join("bashrc");
        let mut config = AppConfig::default();
        config.shells.bash_rc_path = Some(profile.clone());
        config.save(&paths).unwrap();
        let database = crate::storage::Database::open(root.join("aliases.db")).unwrap();
        database
            .insert_alias(&AliasRecord {
                name: "gs".into(),
                executable: "git".into(),
                shells: vec![ShellKind::Bash],
                ..Default::default()
            })
            .unwrap();
        drop(database);

        sync_config_dir(&root).unwrap();
        let database = crate::storage::Database::open(root.join("aliases.db")).unwrap();
        let state = database
            .shell_states()
            .unwrap()
            .into_iter()
            .find(|state| state.shell == ShellKind::Bash)
            .unwrap();
        assert!(
            !state.loader_installed,
            "a clean profile has no loader; the state must say so"
        );
        drop(database);

        crate::integration::install_loader(&paths, "bash").unwrap();
        sync_config_dir(&root).unwrap();
        let database = crate::storage::Database::open(root.join("aliases.db")).unwrap();
        let state = database
            .shell_states()
            .unwrap()
            .into_iter()
            .find(|state| state.shell == ShellKind::Bash)
            .unwrap();
        assert!(state.loader_installed);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn collect_io_mode_returns_one_failed_row_per_shell_instead_of_aborting() {
        let root = sync_root("collect-io");
        let database = crate::storage::Database::open(root.join("aliases.db")).unwrap();
        database
            .insert_alias(&AliasRecord {
                name: "multi".into(),
                executable: "git".into(),
                shells: vec![ShellKind::Bash, ShellKind::Zsh],
                ..Default::default()
            })
            .unwrap();
        drop(database);
        // Force every generated write to fail.
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("generated"), "not a directory").unwrap();

        assert!(
            sync_config_dir(&root).is_err(),
            "the strict CLI path keeps propagating the I/O error"
        );
        let run = sync_config_dir_collect_io(&root).unwrap();
        assert_eq!(run.results.len(), 2);
        assert!(run.failed());
        for result in &run.results {
            assert_eq!(result.status, ShellStatus::Failed);
            assert_eq!(result.error_code.as_deref(), Some("sync_io"));
            assert!(result.error.is_some());
        }
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(windows)]
    #[test]
    fn collect_io_mode_keeps_a_successful_shell_when_another_shell_write_fails() {
        let root = sync_root("collect-partial-io");
        let database = crate::storage::Database::open(root.join("aliases.db")).unwrap();
        database
            .insert_alias(&AliasRecord {
                name: "multi".into(),
                executable: "git".into(),
                shells: vec![ShellKind::Bash, ShellKind::Zsh],
                ..Default::default()
            })
            .unwrap();
        drop(database);
        sync_config_dir(&root).unwrap();

        let bash_file = root.join("generated/bash.sh");
        let mut permissions = fs::metadata(&bash_file).unwrap().permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&bash_file, permissions).unwrap();
        let run = sync_config_dir_collect_io(&root);
        let mut permissions = fs::metadata(&bash_file).unwrap().permissions();
        #[allow(clippy::permissions_set_readonly_false)]
        permissions.set_readonly(false);
        fs::set_permissions(&bash_file, permissions).unwrap();

        let run = run.unwrap();
        assert_eq!(run.results.len(), 2, "both shell rows survive a failure");
        let bash = run
            .results
            .iter()
            .find(|result| result.shell == ShellKind::Bash)
            .unwrap();
        let zsh = run
            .results
            .iter()
            .find(|result| result.shell == ShellKind::Zsh)
            .unwrap();
        assert_eq!(bash.status, ShellStatus::Failed);
        assert!(matches!(
            bash.error_code.as_deref(),
            Some("sync_permission") | Some("sync_io")
        ));
        assert_eq!(zsh.status, ShellStatus::Ok);
        assert!(zsh.error_code.is_none());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn shared_profile_marks_the_shell_whose_block_is_absent() {
        let root = sync_root("shared-loader");
        let paths = AppPaths { root: root.clone() };
        let shared = root.join("shared.rc");
        let mut config = AppConfig::default();
        config.shells.bash_rc_path = Some(shared.clone());
        config.shells.zsh_rc_path = Some(shared.clone());
        config.save(&paths).unwrap();
        let database = crate::storage::Database::open(root.join("aliases.db")).unwrap();
        database
            .insert_alias(&AliasRecord {
                name: "both".into(),
                executable: "git".into(),
                shells: vec![ShellKind::Bash, ShellKind::Zsh],
                ..Default::default()
            })
            .unwrap();
        drop(database);
        crate::integration::install_loader(&paths, "bash").unwrap();

        let run = sync_config_dir_collect_io(&root).unwrap();
        let bash = run
            .results
            .iter()
            .find(|result| result.shell == ShellKind::Bash)
            .unwrap();
        let zsh = run
            .results
            .iter()
            .find(|result| result.shell == ShellKind::Zsh)
            .unwrap();
        assert!(bash.loader_installed);
        assert!(bash.loader_error_code.is_none());
        assert!(!zsh.loader_installed);
        assert_eq!(
            zsh.loader_error_code.as_deref(),
            Some("loader_shared_path"),
            "a shared profile must not produce a generic install hint"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn syntax_failures_carry_the_generic_sync_failed_code() {
        let root = sync_root("failure-code");
        let database = crate::storage::Database::open(root.join("aliases.db")).unwrap();
        database
            .insert_alias(&AliasRecord {
                name: "bad".into(),
                executable: "git".into(),
                shells: vec![ShellKind::Zsh],
                advanced_shell_mode: true,
                ..Default::default()
            })
            .unwrap();
        drop(database);

        let run = sync_config_dir(&root).unwrap();
        assert_eq!(run.results[0].status, ShellStatus::Failed);
        assert_eq!(run.results[0].error_code.as_deref(), Some("sync_failed"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn journal_recovery_predicate_only_accepts_interrupted_states() {
        let root = sync_root("journal-predicate");
        fs::create_dir_all(&root).unwrap();
        let path = root.join("operation.journal");

        // Missing journal: nothing to recover.
        assert!(!journal_needs_recovery(&path).unwrap());

        for (state, expected) in [
            ("prepared", true),
            ("committed", true),
            ("completed", false),
        ] {
            fs::write(
                &path,
                format!("revision_from=1\nrevision_to=2\nstate={state}\nbackups_json=[]\n"),
            )
            .unwrap();
            assert_eq!(
                journal_needs_recovery(&path).unwrap(),
                expected,
                "state {state}"
            );
        }

        fs::write(&path, "revision_to=2\nbackups_json=[]\n").unwrap();
        assert!(matches!(
            journal_needs_recovery(&path),
            Err(AliasError::Config(message)) if message.contains("no state")
        ));

        fs::write(&path, "state=half-written\n").unwrap();
        assert!(matches!(
            journal_needs_recovery(&path),
            Err(AliasError::Config(message)) if message.contains("unknown")
        ));

        // A directory cannot be read as a journal.
        fs::remove_file(&path).unwrap();
        fs::create_dir(&path).unwrap();
        assert!(journal_needs_recovery(&path).is_err());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn shell_content_status_detects_a_delete_that_does_not_advance_the_global_revision() {
        use crate::service::AliasService;
        let root = sync_root("revision-skew");
        let database = crate::storage::Database::open(root.join("aliases.db")).unwrap();
        let service = AliasService::new(&database);
        service
            .create(AliasRecord {
                name: "a".into(),
                executable: "git".into(),
                shells: vec![ShellKind::Bash],
                ..Default::default()
            })
            .unwrap();
        let mut b = service
            .create(AliasRecord {
                name: "b".into(),
                executable: "git".into(),
                shells: vec![ShellKind::Bash],
                ..Default::default()
            })
            .unwrap();
        // Make b revision 5 so the tombstone for a (revision 2) cannot advance
        // the global derived revision after the delete.
        for index in 0..4 {
            b.description = format!("edit {index}");
            b = service.update(b).unwrap();
        }
        assert_eq!(b.revision, 5);
        sync_config_dir(&root).unwrap();

        let aliases = database.list_aliases().unwrap();
        let revision = current_revision(&database, &aliases).unwrap();
        assert_eq!(revision, 5);
        let before =
            shell_content_status(&root, &database, &aliases, &ShellKind::Bash, revision).unwrap();
        assert!(before.is_up_to_date(), "fresh sync must match");

        service.delete("a").unwrap();
        let aliases = database.list_aliases().unwrap();
        let revision = current_revision(&database, &aliases).unwrap();
        assert_eq!(
            revision, 5,
            "the delete tombstone must not advance max(rev)"
        );
        let after =
            shell_content_status(&root, &database, &aliases, &ShellKind::Bash, revision).unwrap();
        assert_eq!(after.state, ShellContentState::Modified);
        assert!(!after.is_up_to_date());
        assert!(after
            .detail
            .as_deref()
            .unwrap_or_default()
            .contains("expected checksum"));
        let generated = fs::read_to_string(root.join("generated/bash.sh")).unwrap();
        assert!(
            generated.contains("a() {"),
            "old definition is still present"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn subset_sync_only_touches_the_requested_shells() {
        let root = sync_root("subset");
        let database = crate::storage::Database::open(root.join("aliases.db")).unwrap();
        database
            .insert_alias(&AliasRecord {
                name: "gs".into(),
                executable: "git".into(),
                shells: vec![ShellKind::Bash, ShellKind::Zsh],
                ..Default::default()
            })
            .unwrap();
        drop(database);
        sync_config_dir(&root).unwrap();
        let bash_path = root.join("generated/bash.sh");
        let zsh_path = root.join("generated/zsh.sh");
        let bash_before = fs::read_to_string(&bash_path).unwrap();
        let zsh_before = fs::read_to_string(&zsh_path).unwrap();

        let retry = sync_shells(&root, &[ShellKind::Zsh]).unwrap();
        assert_eq!(retry.results.len(), 1);
        assert_eq!(retry.results[0].shell, ShellKind::Zsh);
        assert_eq!(retry.results[0].status, ShellStatus::Ok);
        assert_eq!(fs::read_to_string(&bash_path).unwrap(), bash_before);
        assert!(fs::read_to_string(&zsh_path).unwrap().contains("gs"));
        let _ = zsh_before;
        let _ = fs::remove_dir_all(root);
    }
}
