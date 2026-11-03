//! Read models and background jobs.
//!
//! Nothing here re-implements business rules: aliases come from the shared
//! `Database`, sync from `sync::sync_config_dir_collect_io`, diagnostics from
//! `doctor::diagnose`, loader work from `integration`, and previews from
//! `SyncCoordinator::render_shell`.

use aliasmgr_core::{
    config::AppPaths,
    doctor::{diagnose, DoctorFinding},
    error::AliasError,
    fsutil,
    integration::{self, LoaderOutcome},
    model::{AliasRecord, ManagedNameSet, ShellKind, ShellState, ShellStatus},
    shells,
    storage::Database,
    sync::{self, ShellContentState, ShellContentStatus, SyncRun},
    transfer::{self, ConflictStrategy, ImportReport},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, Sender, TryRecvError},
    time::SystemTime,
};

/// Everything the UI needs for one refresh. This is a cache of database state,
/// never the source of truth: every action re-reads the database.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub aliases: Vec<AliasRecord>,
    pub shell_states: Vec<ShellState>,
    pub loaders: BTreeMap<ShellKind, LoaderInfo>,
    pub retired: BTreeMap<ShellKind, Vec<String>>,
    /// Expected-vs-actual generated content per shell targeted by sync,
    /// computed once per refresh so the header and detail share one verdict.
    /// Orphan files and shell_state-only shells are deliberately excluded; they
    /// cannot be cleared by sync and are Doctor's manual-cleanup concern.
    pub content: BTreeMap<ShellKind, ShellContentStatus>,
    pub revision: i64,
    pub fingerprint: DiskFingerprint,
    pub db_modified: Option<SystemTime>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoaderInfo {
    pub target: Option<PathBuf>,
    pub installed: bool,
    pub error_code: Option<String>,
}

/// Cheap change detector for external CLI/GUI writes. Reading it does not open
/// or create a database, so the TUI can poll it while idle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiskFingerprint {
    pub max_revision: i64,
    pub alias_count: i64,
    pub max_updated: String,
    pub max_retired: i64,
    pub shell_state_count: i64,
    pub max_applied: i64,
}

pub fn load_snapshot(database: &Database, root: &Path) -> Result<Snapshot, AliasError> {
    let aliases = database.list_aliases()?;
    let shell_states = database.shell_states()?;
    let revision = sync::current_revision(database, &aliases)?;
    let fingerprint = disk_fingerprint(database)?;
    let db_modified = std::fs::metadata(root.join("aliases.db"))
        .ok()
        .and_then(|metadata| metadata.modified().ok());

    // Only shells that core sync actually targets (alias-bound plus
    // tombstoned shells) determine the header verdict. Leftover generated files
    // and shell_state-only shells cannot be cleared by `sync`; Doctor surfaces
    // those as manual cleanup instead.
    let sync_referenced: BTreeSet<ShellKind> = sync::referenced_shells(database, &aliases)?
        .into_iter()
        .collect();
    let mut referenced = sync_referenced.iter().cloned().collect::<Vec<_>>();
    for state in &shell_states {
        if !referenced.contains(&state.shell) {
            referenced.push(state.shell.clone());
        }
    }
    let paths = AppPaths {
        root: root.to_path_buf(),
    };
    let mut retired = BTreeMap::new();
    let mut loaders = BTreeMap::new();
    let mut content = BTreeMap::new();
    for shell in referenced {
        if sync_referenced.contains(&shell) {
            content.insert(
                shell.clone(),
                sync::shell_content_status(root, database, &aliases, &shell, revision)?,
            );
        }
        retired.insert(shell.clone(), database.managed_name_set(&shell)?.retired);
        let info = match integration::loader_status(&paths, shell.as_str()) {
            Ok(outcome) => LoaderInfo {
                target: Some(outcome.target),
                installed: outcome.installed,
                error_code: None,
            },
            Err(AliasError::ShellNotInstalled) => LoaderInfo {
                target: None,
                installed: false,
                error_code: Some("loader_unsupported".to_string()),
            },
            Err(error) => LoaderInfo {
                target: None,
                installed: false,
                error_code: Some(integration::loader_error_code(&error).to_string()),
            },
        };
        loaders.insert(shell, info);
    }
    Ok(Snapshot {
        aliases,
        shell_states,
        loaders,
        retired,
        content,
        revision,
        fingerprint,
        db_modified,
    })
}

pub fn disk_fingerprint(database: &Database) -> Result<DiskFingerprint, AliasError> {
    let (max_revision, alias_count, max_updated) = database.conn.query_row(
        "SELECT COALESCE(MAX(revision),0), COUNT(*), COALESCE(MAX(updated_at),'') FROM aliases",
        [],
        |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
            ))
        },
    )?;
    let max_retired = database.conn.query_row(
        "SELECT COALESCE(MAX(retired_at_revision),0) FROM retired_names",
        [],
        |row| row.get::<_, i64>(0),
    )?;
    let (shell_state_count, max_applied) = database.conn.query_row(
        "SELECT COUNT(*), COALESCE(MAX(applied_revision),0) FROM shell_state",
        [],
        |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
    )?;
    Ok(DiskFingerprint {
        max_revision,
        alias_count,
        max_updated,
        max_retired,
        shell_state_count,
        max_applied,
    })
}

/// Layer 2: whether the generated file reflects the stored record.
pub fn generated_layer(snapshot: &Snapshot, alias: &AliasRecord, shell: &ShellKind) -> String {
    let shell_state = snapshot
        .shell_states
        .iter()
        .find(|state| state.shell == *shell);
    if !alias.enabled {
        return match snapshot.content.get(shell) {
            Some(status)
                if status.is_up_to_date() || status.state == ShellContentState::Missing =>
            {
                "not generated (alias disabled)".to_string()
            }
            Some(_) => "not generated (alias disabled); generated file needs sync".to_string(),
            None => "not generated (alias disabled)".to_string(),
        };
    }
    if let Some(state) = shell_state {
        if state.status == ShellStatus::Failed {
            return format!(
                "failed: {}",
                state.last_error.as_deref().unwrap_or("unknown error")
            );
        }
    }
    let Some(status) = snapshot.content.get(shell) else {
        return "not synced yet".to_string();
    };
    match status.state {
        ShellContentState::UpToDate => "up to date".to_string(),
        ShellContentState::Missing if shell_state.is_none() => "not synced yet".to_string(),
        ShellContentState::Missing => "file missing (needs sync)".to_string(),
        ShellContentState::Unreadable => "file unreadable (run Doctor)".to_string(),
        ShellContentState::RenderFailed => "render failed (run Doctor)".to_string(),
        ShellContentState::Modified => {
            "stale content or checksum mismatch (needs sync / run Doctor)".to_string()
        }
    }
}

/// Layer 3: whether the marked loader block is installed in the profile.
pub fn loader_layer(snapshot: &Snapshot, shell: &ShellKind) -> String {
    let Some(info) = snapshot.loaders.get(shell) else {
        return "not checked".to_string();
    };
    if let Some(code) = &info.error_code {
        return match code.as_str() {
            "loader_shared_path" => "shared profile: only one shell can load".to_string(),
            "loader_unsupported" => "no profile for this shell on this platform".to_string(),
            other => format!("error ({other})"),
        };
    }
    if info.installed {
        match &info.target {
            Some(path) => format!("installed ({})", path.display()),
            None => "installed".to_string(),
        }
    } else {
        "not installed".to_string()
    }
}

/// Builds the four status layers for one alias. Layer 4 is always unknown:
/// there is no reliable way to inspect the shell that launched the TUI.
pub fn four_layer_lines(snapshot: &Snapshot, alias: &AliasRecord) -> Vec<String> {
    let mut lines = Vec::new();
    lines.push(format!("1 DB: saved (revision {})", alias.revision));
    for shell in &alias.shells {
        lines.push(format!(
            "2 generated [{}]: {}",
            shell.as_str(),
            generated_layer(snapshot, alias, shell)
        ));
        lines.push(format!(
            "3 loader [{}]: {}",
            shell.as_str(),
            loader_layer(snapshot, shell)
        ));
    }
    lines.push("4 current shell: unknown (reload is never automatic)".to_string());
    lines
}

/// Renders a shell's generated body from the supplied records. Environment
/// values are replaced with `***` before rendering so a preview cannot leak a
/// secret; the title shown by the UI states that values are masked.
pub fn preview_shell(
    root: &Path,
    snapshot: &Snapshot,
    shell: &ShellKind,
    aliases: &[AliasRecord],
) -> Result<String, AliasError> {
    let redacted: Vec<AliasRecord> = aliases
        .iter()
        .cloned()
        .map(|mut alias| {
            let redacted = crate::text::redact_environment(alias.environment.iter());
            alias.environment = redacted.into_iter().collect();
            alias
        })
        .collect();
    let shell_aliases: Vec<AliasRecord> = redacted
        .into_iter()
        .filter(|alias| alias.enabled && alias.shells.contains(shell))
        .collect();
    let names = ManagedNameSet {
        current: shell_aliases
            .iter()
            .map(|alias| alias.name.clone())
            .collect(),
        retired: snapshot.retired.get(shell).cloned().unwrap_or_default(),
    };
    sync::SyncCoordinator::new(root).render_shell(&shell_aliases, shell, snapshot.revision, &names)
}

/// Renders just one alias's definition through the existing Shell adapter, with
/// every environment value masked before rendering. Unrelated aliases cannot
/// make this preview fail.
pub fn preview_alias(alias: &AliasRecord, shell: &ShellKind) -> Result<String, AliasError> {
    let mut redacted = alias.clone();
    for value in redacted.environment.values_mut() {
        *value = "***".to_string();
    }
    shells::render_alias(&redacted, shell)
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileFingerprint {
    pub len: u64,
    pub modified_nanos: Option<u128>,
    /// Strong content identity; size/mtime alone can be bypassed by a
    /// same-size edit with a restored timestamp.
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportReportSummary {
    pub imported: Vec<String>,
    pub skipped: Vec<String>,
    pub unsupported: Vec<String>,
    pub warnings: Vec<String>,
    pub strategy: ConflictStrategy,
    pub path: PathBuf,
    pub format: &'static str,
    /// Exact-name -> id snapshot from the preview database. Carried to
    /// confirmation so only rows the user actually previewed as overwrites can
    /// be updated; rows created/replaced in between become conflicts.
    pub overwrite_existing: BTreeMap<String, uuid::Uuid>,
    /// Size and modification time of the previewed file; confirmation refuses
    /// to import if it changed after the preview.
    pub file_fingerprint: FileFingerprint,
}

impl ImportReportSummary {
    fn from_report(
        report: ImportReport,
        strategy: ConflictStrategy,
        path: PathBuf,
        format: &'static str,
        overwrite_existing: BTreeMap<String, uuid::Uuid>,
        file_fingerprint: FileFingerprint,
    ) -> Self {
        Self {
            imported: report.imported,
            skipped: report.skipped,
            unsupported: report.unsupported,
            warnings: report.warnings,
            strategy,
            path,
            format,
            overwrite_existing,
            file_fingerprint,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportSummary {
    pub count: usize,
    pub path: PathBuf,
    pub format: &'static str,
}

pub fn transfer_format(path: &Path) -> &'static str {
    if path.extension().and_then(|value| value.to_str()) == Some("toml") {
        "TOML"
    } else {
        "JSON"
    }
}

fn parse_import_records(path: &Path, content: &str) -> Result<Vec<AliasRecord>, AliasError> {
    if transfer_format(path) == "TOML" {
        transfer::read_toml_str(content)
    } else {
        transfer::read_json_str(content)
    }
}

fn existing_aliases_for_preview(root: &Path) -> Result<BTreeMap<String, AliasRecord>, AliasError> {
    let database = root.join("aliases.db");
    if !database.exists() {
        return Ok(BTreeMap::new());
    }
    Ok(Database::open_read_only(database)?
        .list_aliases()?
        .into_iter()
        .map(|alias| (alias.name.clone(), alias))
        .collect())
}

/// Applies the same PowerShell case-fold rule that core uses, but for the
/// Overwrite strategy where core accepts the record. `existing` is the
/// snapshot used for preview or confirmation, so both report identical counts.
fn filter_overwrite_casefold(report: &mut ImportReport, existing: &BTreeMap<String, AliasRecord>) {
    let mut accepted = Vec::new();
    let mut imported = Vec::new();
    for record in report.accepted_records.drain(..) {
        let exact_id = existing.get(&record.name).map(|alias| alias.id);
        if transfer::powershell_casefold_conflict(&record.name, &record.shells, existing, exact_id)
        {
            report.warnings.push(format!(
                "PowerShell case-fold conflict requires manual resolution: {}",
                record.name
            ));
            report.skipped.push(record.name);
        } else {
            imported.push(record.name.clone());
            accepted.push(record);
        }
    }
    report.accepted_records = accepted;
    report.imported = imported;
}

fn import_error(error: &AliasError, path: &Path) -> String {
    match error {
        AliasError::Io(io) if io.kind() == std::io::ErrorKind::NotFound => {
            format!("file not found: {}", path.display())
        }
        AliasError::Io(io) if io.kind() == std::io::ErrorKind::PermissionDenied => {
            format!("permission denied reading {}", path.display())
        }
        AliasError::Io(io)
            if io.kind() == std::io::ErrorKind::IsADirectory
                || io.kind() == std::io::ErrorKind::NotADirectory =>
        {
            format!("path is a directory: {}", path.display())
        }
        AliasError::Config(message) => message.clone(),
        AliasError::Serialization(error) => format!("invalid JSON export: {error}"),
        _ => format!("could not import {}: {error}", path.display()),
    }
}

fn export_error(error: &AliasError, path: &Path) -> String {
    match error {
        AliasError::Io(io) if io.kind() == std::io::ErrorKind::NotFound => {
            let parent = path.parent().unwrap_or_else(|| Path::new("."));
            format!("target directory does not exist: {}", parent.display())
        }
        AliasError::Io(io) if io.kind() == std::io::ErrorKind::PermissionDenied => {
            format!("permission denied writing {}", path.display())
        }
        AliasError::Io(io)
            if io.kind() == std::io::ErrorKind::IsADirectory
                || io.kind() == std::io::ErrorKind::NotADirectory =>
        {
            format!("target is a directory: {}", path.display())
        }
        AliasError::Config(message) => message.clone(),
        AliasError::Serialization(error) => format!("could not serialize export: {error}"),
        _ => format!("could not export to {}: {error}", path.display()),
    }
}

fn modified_nanos(metadata: &std::fs::Metadata) -> Option<u128> {
    metadata
        .modified()
        .ok()
        .and_then(|value| value.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos())
}

pub fn file_fingerprint(path: &Path) -> Result<FileFingerprint, String> {
    if path.is_dir() {
        return Err(format!("path is a directory: {}", path.display()));
    }
    let metadata =
        std::fs::metadata(path).map_err(|error| import_error(&AliasError::Io(error), path))?;
    let sha256 = fsutil::file_sha256(path).map_err(|error| import_error(&error, path))?;
    Ok(FileFingerprint {
        len: metadata.len(),
        modified_nanos: modified_nanos(&metadata),
        sha256,
    })
}

/// Reads and hashes the import file exactly once: the SHA-256 is computed over
/// the same bytes that are parsed. Windows callers pass the preview fingerprint
/// (checked before the read and against the read bytes); preview passes `None`
/// and receives the fingerprint to carry forward.
fn read_checked_import_records(
    path: &Path,
    expected: Option<&FileFingerprint>,
    before_read: impl FnOnce(),
) -> Result<(Vec<AliasRecord>, FileFingerprint), String> {
    if path.is_dir() {
        return Err(format!("path is a directory: {}", path.display()));
    }
    if let Some(expected) = expected {
        if file_fingerprint(path)? != *expected {
            return Err(
                "import file changed since the preview; preview it again before importing"
                    .to_string(),
            );
        }
    }
    before_read();
    let read_error = |error| import_error(&AliasError::Io(error), path);
    let before = std::fs::metadata(path).map_err(read_error)?;
    let bytes = std::fs::read(path).map_err(read_error)?;
    let after = std::fs::metadata(path).map_err(read_error)?;
    let fingerprint = FileFingerprint {
        len: bytes.len() as u64,
        modified_nanos: modified_nanos(&after),
        sha256: fsutil::sha256_hex(&bytes),
    };
    let changed = match expected {
        Some(expected) => fingerprint != *expected,
        // Preview has no expected fingerprint, so compare the metadata captured
        // around the read to detect a concurrent edit while it was read.
        None => {
            before.len() != fingerprint.len || modified_nanos(&before) != fingerprint.modified_nanos
        }
    };
    if changed {
        return Err(match expected {
            Some(_) => {
                "import file changed while it was being read; preview it again before importing"
                    .to_string()
            }
            None => "import file changed while it was being previewed; try again".to_string(),
        });
    }
    let content = String::from_utf8(bytes)
        .map_err(|error| read_error(std::io::Error::new(std::io::ErrorKind::InvalidData, error)))?;
    let records =
        parse_import_records(path, &content).map_err(|error| import_error(&error, path))?;
    Ok((records, fingerprint))
}

/// Read-only preview: parses and evaluates conflicts without creating the
/// database or writing any file. `ImportReport::accepted_records` is dropped.
pub fn import_preview(
    root: &Path,
    path: &Path,
    strategy: ConflictStrategy,
) -> Result<ImportReportSummary, String> {
    let (records, fingerprint) = read_checked_import_records(path, None, || {})?;
    let existing =
        existing_aliases_for_preview(root).map_err(|error| import_error(&error, path))?;
    transfer::import_records(records, &existing, strategy)
        .map(|mut report| {
            let overwrite_existing = if strategy == ConflictStrategy::Overwrite {
                filter_overwrite_casefold(&mut report, &existing);
                existing
                    .iter()
                    .map(|(name, alias)| (name.clone(), alias.id))
                    .collect::<BTreeMap<_, _>>()
            } else {
                BTreeMap::new()
            };
            ImportReportSummary::from_report(
                report.into_public(),
                strategy,
                path.to_path_buf(),
                transfer_format(path),
                overwrite_existing,
                fingerprint,
            )
        })
        .map_err(|error| import_error(&error, path))
}

/// Re-evaluates against the current database at confirmation time and inserts
/// the accepted records in one transaction.
pub fn import_confirm(
    root: &Path,
    path: &Path,
    strategy: ConflictStrategy,
    preview_existing: &BTreeMap<String, uuid::Uuid>,
    preview_fingerprint: &FileFingerprint,
) -> Result<ImportReportSummary, String> {
    import_confirm_after_read(
        root,
        path,
        strategy,
        preview_existing,
        preview_fingerprint,
        || {},
    )
}

fn import_confirm_after_read(
    root: &Path,
    path: &Path,
    strategy: ConflictStrategy,
    preview_existing: &BTreeMap<String, uuid::Uuid>,
    preview_fingerprint: &FileFingerprint,
    after_read: impl FnOnce(),
) -> Result<ImportReportSummary, String> {
    let (records, verified) = read_checked_import_records(path, Some(preview_fingerprint), || {})?;
    after_read();
    // Check before any writable open: a changed file must not create an empty
    // database (or its backup) merely because confirmation was attempted.
    if file_fingerprint(path)? != *preview_fingerprint {
        return Err(
            "import file changed before the import transaction; preview it again".to_string(),
        );
    }
    let database =
        Database::open(root.join("aliases.db")).map_err(|error| import_error(&error, path))?;
    let existing = database
        .list_aliases()
        .map_err(|error| import_error(&error, path))?
        .into_iter()
        .map(|alias| (alias.name.clone(), alias))
        .collect::<BTreeMap<_, _>>();
    let mut report = transfer::import_records(records, &existing, strategy)
        .map_err(|error| import_error(&error, path))?;
    let overwrite = strategy == ConflictStrategy::Overwrite;
    if overwrite {
        // Core's Overwrite strategy accepts the record, but exact-name rows
        // must update the existing id rather than INSERT. Resolve remaining
        // PowerShell case-fold conflicts before writing instead of failing the
        // whole transaction on a unique/case-fold constraint.
        filter_overwrite_casefold(&mut report, &existing);
        // Only rows that existed at preview time (same id) may be updated.
        // A row created or replaced in between is a conflict, not an overwrite.
        let mut accepted = Vec::new();
        let mut imported = Vec::new();
        for record in report.accepted_records.drain(..) {
            let current_id = existing.get(&record.name).map(|alias| alias.id);
            let previewed_id = preview_existing.get(&record.name).copied();
            let safe = match (previewed_id, current_id) {
                (Some(preview), Some(current)) => preview == current,
                (Some(_), None) => true, // row was deleted; insert the imported record
                (None, Some(_)) => false,
                (None, None) => true,
            };
            if safe {
                imported.push(record.name.clone());
                accepted.push(record);
            } else {
                report.skipped.push(record.name.clone());
                report.warnings.push(format!(
                    "name was created or replaced after preview; skipped: {}",
                    record.name
                ));
            }
        }
        report.accepted_records = accepted;
        report.imported = imported;
    }
    let overwrite_existing = if overwrite {
        preview_existing.clone()
    } else {
        BTreeMap::new()
    };
    database
        .import_aliases_atomic(&report.accepted_records, &overwrite_existing)
        .map_err(|error| import_error(&error, path))?;
    Ok(ImportReportSummary::from_report(
        report.into_public(),
        strategy,
        path.to_path_buf(),
        transfer_format(path),
        overwrite_existing,
        verified,
    ))
}

pub fn export_aliases(root: &Path, path: &Path, overwrite: bool) -> Result<ExportSummary, String> {
    if path.is_dir() {
        return Err(format!("target is a directory: {}", path.display()));
    }
    if path.exists() && !overwrite {
        return Err(format!(
            "file already exists; overwrite was not confirmed: {}",
            path.display()
        ));
    }
    let database = root.join("aliases.db");
    let aliases = if database.exists() {
        Database::open_read_only(&database)
            .and_then(|database| database.list_aliases())
            .map_err(|error| export_error(&error, path))?
    } else {
        Vec::new()
    };
    let format = transfer_format(path);
    let result = if format == "TOML" {
        transfer::export_toml(path, &aliases)
    } else {
        transfer::export_json(path, &aliases)
    };
    result.map_err(|error| export_error(&error, path))?;
    Ok(ExportSummary {
        count: aliases.len(),
        path: path.to_path_buf(),
        format,
    })
}

/// Inserts or replaces a record in a preview list without touching the DB.
pub fn with_draft(aliases: &[AliasRecord], draft: Option<&AliasRecord>) -> Vec<AliasRecord> {
    let Some(draft) = draft else {
        return aliases.to_vec();
    };
    let mut result = aliases.to_vec();
    match result.iter().position(|alias| alias.id == draft.id) {
        Some(index) => result[index] = draft.clone(),
        None => result.push(draft.clone()),
    }
    result
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobKind {
    Sync,
    /// Retry only the shells that failed in a previous partial sync.
    SyncRetry(Vec<ShellKind>),
    Doctor,
    ImportPreview {
        path: PathBuf,
        strategy: ConflictStrategy,
    },
    ImportConfirm {
        path: PathBuf,
        strategy: ConflictStrategy,
        overwrite_existing: BTreeMap<String, uuid::Uuid>,
        file_fingerprint: FileFingerprint,
    },
    Export {
        path: PathBuf,
        overwrite: bool,
    },
    LoaderInstall(String),
    LoaderUninstall(String),
}

impl JobKind {
    pub fn label(&self) -> String {
        match self {
            JobKind::Sync => "syncing".to_string(),
            JobKind::SyncRetry(shells) => format!("retrying {} failed shell(s)", shells.len()),
            JobKind::Doctor => "running doctor".to_string(),
            JobKind::ImportPreview { .. } => "previewing import".to_string(),
            JobKind::ImportConfirm { .. } => "importing aliases".to_string(),
            JobKind::Export { .. } => "exporting aliases".to_string(),
            JobKind::LoaderInstall(shell) => format!("installing {shell} loader"),
            JobKind::LoaderUninstall(shell) => format!("removing {shell} loader"),
        }
    }
}

#[derive(Debug)]
pub enum JobResult {
    Sync(Result<SyncRun, String>),
    Doctor(Result<Vec<DoctorFinding>, String>),
    ImportPreview(Result<ImportReportSummary, String>),
    ImportConfirm(Result<ImportReportSummary, String>),
    Export(Result<ExportSummary, String>),
    Loader(Result<LoaderOutcome, String>),
    /// The worker panicked or its channel disappeared before it could report.
    WorkerFailed(String),
}

#[derive(Debug)]
pub struct JobOutcome {
    pub kind: JobKind,
    pub result: JobResult,
}

/// One background job at a time. Writes are never concurrent with each other;
/// sync itself holds the shared core sync lock.
pub struct Jobs {
    sender: Option<Sender<JobOutcome>>,
    receiver: Receiver<JobOutcome>,
    running: Option<JobKind>,
}

impl Default for Jobs {
    fn default() -> Self {
        Self::new()
    }
}

impl Jobs {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel();
        Self {
            sender: Some(sender),
            receiver,
            running: None,
        }
    }

    pub fn is_busy(&self) -> bool {
        self.running.is_some()
    }

    pub fn running(&self) -> Option<&JobKind> {
        self.running.as_ref()
    }

    /// Starts a job unless one is already running. Returns false when busy so
    /// callers can keep the current UI and show a message instead of queueing
    /// overlapping writes.
    pub fn start(&mut self, kind: JobKind, root: PathBuf) -> bool {
        if self.running.is_some() {
            return false;
        }
        let Some(sender) = self.sender.clone() else {
            return false;
        };
        let thread_kind = kind.clone();
        let spawned = std::thread::Builder::new()
            .name(format!("aliasmgr-tui-{}", thread_kind.label()))
            .spawn(move || {
                let result = catch_worker(|| run_job(&thread_kind, &root));
                let _ = sender.send(JobOutcome {
                    kind: thread_kind,
                    result,
                });
            });
        match spawned {
            Ok(_) => {
                self.running = Some(kind);
                true
            }
            Err(_) => false,
        }
    }

    pub fn try_recv(&mut self) -> Option<JobOutcome> {
        match self.receiver.try_recv() {
            Ok(outcome) => {
                self.running = None;
                Some(outcome)
            }
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => self.running.take().map(|kind| JobOutcome {
                kind,
                result: JobResult::WorkerFailed(
                    "background worker disconnected before reporting".to_string(),
                ),
            }),
        }
    }

    /// Test-only marker for a busy worker without spawning a thread.
    #[cfg(test)]
    pub fn mark_running_for_test(&mut self, kind: JobKind) {
        self.running = Some(kind);
    }

    /// Test-only injection of a worker outcome for deterministic UI tests.
    #[cfg(test)]
    pub fn inject(&mut self, outcome: JobOutcome) {
        if let Some(sender) = &self.sender {
            let _ = sender.send(outcome);
        }
    }

    /// Test-only blocking receive with a deadline.
    #[cfg(test)]
    pub fn wait(&mut self, timeout: std::time::Duration) -> Option<JobOutcome> {
        let outcome = self.receiver.recv_timeout(timeout).ok();
        if outcome.is_some() {
            self.running = None;
        }
        outcome
    }
}

/// Runs a worker body, converting a panic into a result the UI can display.
/// Without this a panicking sync/doctor thread would leave `running` set and
/// the TUI would refuse every later write forever.
fn catch_worker<F: FnOnce() -> JobResult>(work: F) -> JobResult {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(work)) {
        Ok(result) => result,
        Err(_) => JobResult::WorkerFailed("background worker panicked".to_string()),
    }
}

fn run_job(kind: &JobKind, root: &Path) -> JobResult {
    match kind {
        JobKind::Sync => JobResult::Sync(
            sync::sync_config_dir_collect_io(root).map_err(|error| error.to_string()),
        ),
        JobKind::SyncRetry(shells) => {
            JobResult::Sync(sync::sync_shells(root, shells).map_err(|error| error.to_string()))
        }
        JobKind::Doctor => JobResult::Doctor(run_doctor(root)),
        JobKind::ImportPreview { path, strategy } => {
            JobResult::ImportPreview(import_preview(root, path, *strategy))
        }
        JobKind::ImportConfirm {
            path,
            strategy,
            overwrite_existing,
            file_fingerprint,
        } => JobResult::ImportConfirm(import_confirm(
            root,
            path,
            *strategy,
            overwrite_existing,
            file_fingerprint,
        )),
        JobKind::Export { path, overwrite } => {
            JobResult::Export(export_aliases(root, path, *overwrite))
        }
        JobKind::LoaderInstall(shell) => JobResult::Loader(
            integration::install_loader(
                &AppPaths {
                    root: root.to_path_buf(),
                },
                shell,
            )
            .map_err(|error| error.to_string()),
        ),
        JobKind::LoaderUninstall(shell) => JobResult::Loader(
            integration::uninstall_loader(
                &AppPaths {
                    root: root.to_path_buf(),
                },
                shell,
            )
            .map_err(|error| error.to_string()),
        ),
    }
}

fn run_doctor(root: &Path) -> Result<Vec<DoctorFinding>, String> {
    let database_path_exists = root.join("aliases.db").exists();
    let database = Database::open(root.join("aliases.db")).map_err(|error| error.to_string())?;
    let aliases = database.list_aliases().map_err(|error| error.to_string())?;
    diagnose(root, database_path_exists, &database, &aliases).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use aliasmgr_core::model::{record_checksum, AliasRecord};
    use std::time::Duration;

    fn temp_root(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "aliasmgr-tui-data-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or_default()
        ));
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    fn insert(database: &Database, alias: AliasRecord) -> AliasRecord {
        let mut alias = alias;
        alias.record_checksum = record_checksum(&alias).unwrap();
        database.insert_alias(&alias).unwrap();
        alias
    }

    #[test]
    fn snapshot_reports_layers_for_a_saved_alias() {
        let root = temp_root("layers");
        let database = Database::open(root.join("aliases.db")).unwrap();
        let alias = insert(
            &database,
            AliasRecord {
                name: "gs".into(),
                executable: "git".into(),
                shells: vec![ShellKind::Bash],
                ..Default::default()
            },
        );
        let snapshot = load_snapshot(&database, &root).unwrap();
        assert_eq!(snapshot.revision, alias.revision);
        assert_eq!(
            generated_layer(&snapshot, &alias, &ShellKind::Bash),
            "not synced yet"
        );
        assert!(loader_layer(&snapshot, &ShellKind::Bash).starts_with("not installed"));
        let layers = four_layer_lines(&snapshot, &alias);
        assert!(layers[0].contains("1 DB"));
        assert!(layers
            .iter()
            .any(|line| line.starts_with("2 generated [bash]")));
        assert!(layers
            .iter()
            .any(|line| line.starts_with("3 loader [bash]")));
        assert!(layers.last().unwrap().contains("current shell: unknown"));
        drop(database);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn generated_layer_detects_a_hand_edited_file_even_at_the_synced_revision() {
        let root = temp_root("generated-checksum");
        let database = Database::open(root.join("aliases.db")).unwrap();
        let alias = insert(
            &database,
            AliasRecord {
                name: "gs".into(),
                executable: "git".into(),
                shells: vec![ShellKind::Bash],
                ..Default::default()
            },
        );
        drop(database);
        sync::sync_config_dir(&root).unwrap();
        let database = Database::open(root.join("aliases.db")).unwrap();
        let snapshot = load_snapshot(&database, &root).unwrap();
        assert_eq!(
            generated_layer(&snapshot, &alias, &ShellKind::Bash),
            "up to date"
        );
        let path = root.join("generated/bash.sh");
        let mut content = std::fs::read_to_string(&path).unwrap();
        content.push_str("\n# hand edit\n");
        std::fs::write(&path, content).unwrap();
        let snapshot = load_snapshot(&database, &root).unwrap();
        assert!(generated_layer(&snapshot, &alias, &ShellKind::Bash).contains("checksum mismatch"));
        drop(database);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn snapshot_reports_stale_content_when_a_delete_does_not_advance_the_revision() {
        use aliasmgr_core::service::AliasService;
        let root = temp_root("revision-skew");
        let database = Database::open(root.join("aliases.db")).unwrap();
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
        for index in 0..4 {
            b.description = format!("edit {index}");
            b = service.update(b).unwrap();
        }
        sync::sync_config_dir(&root).unwrap();
        let snapshot = load_snapshot(&database, &root).unwrap();
        assert!(snapshot.content[&ShellKind::Bash].is_up_to_date());

        service.delete("a").unwrap();
        let snapshot = load_snapshot(&database, &root).unwrap();
        assert_eq!(snapshot.revision, 5, "tombstone must not advance max(rev)");
        assert_eq!(
            snapshot.content[&ShellKind::Bash].state,
            ShellContentState::Modified
        );
        assert!(generated_layer(&snapshot, &b, &ShellKind::Bash).contains("needs sync"));
        drop(database);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn preview_masks_environment_values_and_redacts_other_aliases() {
        let root = temp_root("preview");
        let database = Database::open(root.join("aliases.db")).unwrap();
        let mut alias = AliasRecord {
            name: "secret".into(),
            executable: "git".into(),
            shells: vec![ShellKind::Bash],
            ..Default::default()
        };
        alias.environment.insert("TOKEN".into(), "hunter2".into());
        let alias = insert(&database, alias);
        let snapshot = load_snapshot(&database, &root).unwrap();
        let body = preview_shell(
            &root,
            &snapshot,
            &ShellKind::Bash,
            std::slice::from_ref(&alias),
        )
        .unwrap();
        assert!(body.contains("secret"));
        assert!(body.contains("***"));
        assert!(!body.contains("hunter2"));
        drop(database);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn preview_uses_supplied_draft_instead_of_stored_record() {
        let root = temp_root("draft-preview");
        let database = Database::open(root.join("aliases.db")).unwrap();
        let alias = insert(
            &database,
            AliasRecord {
                name: "gs".into(),
                executable: "git".into(),
                shells: vec![ShellKind::Bash],
                ..Default::default()
            },
        );
        let mut draft = alias.clone();
        draft.fixed_args = vec!["status".into()];
        draft.name = "gs2".into();
        let aliases = with_draft(&[alias], Some(&draft));
        let snapshot = load_snapshot(&database, &root).unwrap();
        let body = preview_shell(&root, &snapshot, &ShellKind::Bash, &aliases).unwrap();
        assert!(body.contains("gs2"));
        assert!(!body.contains("alias gs="));
        drop(database);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn fingerprint_changes_after_an_external_write() {
        let root = temp_root("fingerprint");
        let database = Database::open(root.join("aliases.db")).unwrap();
        let before = disk_fingerprint(&database).unwrap();
        let second = Database::open(root.join("aliases.db")).unwrap();
        let mut alias = AliasRecord {
            name: "gs".into(),
            executable: "git".into(),
            ..Default::default()
        };
        alias.record_checksum = record_checksum(&alias).unwrap();
        second.insert_alias(&alias).unwrap();
        let after = disk_fingerprint(&database).unwrap();
        assert_ne!(before, after);
        assert_eq!(after.alias_count, 1);
        drop(second);
        drop(database);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn sync_job_writes_only_the_isolated_config_root() {
        let root = temp_root("sync-job");
        let database = Database::open(root.join("aliases.db")).unwrap();
        insert(
            &database,
            AliasRecord {
                name: "gs".into(),
                executable: "git".into(),
                shells: vec![ShellKind::Bash],
                ..Default::default()
            },
        );
        drop(database);
        let mut jobs = Jobs::new();
        assert!(jobs.start(JobKind::Sync, root.clone()));
        assert!(jobs.is_busy());
        let outcome = jobs
            .wait(Duration::from_secs(10))
            .expect("sync job should finish");
        assert!(!jobs.is_busy());
        match outcome.result {
            JobResult::Sync(Ok(run)) => {
                assert!(run
                    .results
                    .iter()
                    .any(|result| result.shell == ShellKind::Bash));
            }
            other => panic!("unexpected job result: {other:?}"),
        }
        assert!(root.join("generated/bash.sh").exists());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn doctor_job_uses_the_shared_diagnostics() {
        let root = temp_root("doctor-job");
        let database = Database::open(root.join("aliases.db")).unwrap();
        insert(
            &database,
            AliasRecord {
                name: "gs".into(),
                executable: "git".into(),
                shells: vec![ShellKind::Bash],
                ..Default::default()
            },
        );
        drop(database);
        let mut jobs = Jobs::new();
        assert!(jobs.start(JobKind::Doctor, root.clone()));
        let outcome = jobs
            .wait(Duration::from_secs(10))
            .expect("doctor job should finish");
        match outcome.result {
            JobResult::Doctor(Ok(findings)) => {
                // The generated file is missing before the first sync.
                assert!(findings
                    .iter()
                    .any(|finding| finding.code == "generated_missing"));
            }
            other => panic!("unexpected job result: {other:?}"),
        }
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn a_panicking_worker_becomes_a_displayable_failure() {
        let result = catch_worker(|| panic!("worker boom"));
        match result {
            JobResult::WorkerFailed(message) => assert!(message.contains("panicked")),
            other => panic!("unexpected result: {other:?}"),
        }
    }

    #[test]
    fn a_disconnected_worker_clears_busy_and_reports_failure() {
        let mut jobs = Jobs::new();
        jobs.running = Some(JobKind::Sync);
        jobs.sender = None;
        let outcome = jobs.try_recv().expect("must synthesize an outcome");
        assert!(!jobs.is_busy());
        assert_eq!(outcome.kind, JobKind::Sync);
        assert!(matches!(outcome.result, JobResult::WorkerFailed(_)));
    }

    #[test]
    fn second_job_is_refused_while_one_is_running() {
        let root = temp_root("busy");
        Database::open(root.join("aliases.db")).unwrap();
        let mut jobs = Jobs::new();
        assert!(jobs.start(JobKind::Sync, root.clone()));
        assert!(!jobs.start(JobKind::Doctor, root.clone()));
        let _ = jobs.wait(Duration::from_secs(10));
        let _ = std::fs::remove_dir_all(root);
    }

    fn export_file(path: &Path, aliases: &[AliasRecord]) {
        transfer::export_json(path, aliases).unwrap();
    }

    fn write_raw_export(path: &Path, aliases: Vec<AliasRecord>, version: u32) {
        let file = transfer::ExportFile {
            format_version: version,
            exported_at: "2026-01-01T00:00:00+00:00".into(),
            exported_by_version: "test".into(),
            aliases,
        };
        std::fs::write(path, serde_json::to_string(&file).unwrap()).unwrap();
    }

    #[test]
    fn import_preview_is_read_only_and_does_not_create_a_database() {
        let root = temp_root("import-preview");
        let file = root.join("input.json");
        export_file(
            &file,
            &[AliasRecord {
                name: "gs".into(),
                executable: "git".into(),
                shells: vec![ShellKind::Bash],
                ..Default::default()
            }],
        );
        let report = import_preview(&root, &file, ConflictStrategy::Ask).unwrap();
        assert_eq!(report.imported, vec!["gs"]);
        assert_eq!(report.format, "JSON");
        assert!(!root.join("aliases.db").exists());
        assert!(!root.join("generated").exists());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn import_confirm_revalidates_and_persists_only_accepted_records() {
        let root = temp_root("import-confirm");
        let database = Database::open(root.join("aliases.db")).unwrap();
        insert(
            &database,
            AliasRecord {
                name: "existing".into(),
                executable: "tool".into(),
                shells: vec![ShellKind::Bash],
                ..Default::default()
            },
        );
        drop(database);
        let file = root.join("input.json");
        write_raw_export(
            &file,
            vec![
                AliasRecord {
                    name: "one".into(),
                    executable: "tool".into(),
                    shells: vec![ShellKind::Bash],
                    ..Default::default()
                },
                AliasRecord {
                    name: "bad name".into(),
                    executable: "tool".into(),
                    shells: vec![ShellKind::Bash],
                    ..Default::default()
                },
            ],
            1,
        );
        let preview = import_preview(&root, &file, ConflictStrategy::Ask).unwrap();
        assert_eq!(preview.imported, vec!["one"]);
        assert_eq!(preview.skipped, vec!["bad name"]);
        assert!(!root.join("aliases.db").to_string_lossy().is_empty());
        let database = Database::open(root.join("aliases.db")).unwrap();
        assert_eq!(database.list_aliases().unwrap().len(), 1);

        let report = import_confirm(
            &root,
            &file,
            ConflictStrategy::Ask,
            &BTreeMap::new(),
            &file_fingerprint(&file).unwrap(),
        )
        .unwrap();
        assert_eq!(report.imported, vec!["one"]);
        let database = Database::open(root.join("aliases.db")).unwrap();
        let names = database
            .list_aliases()
            .unwrap()
            .into_iter()
            .map(|alias| alias.name)
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["existing", "one"]);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn import_confirm_rejects_a_file_changed_after_preview() {
        let root = temp_root("import-file-change");
        let file = root.join("input.json");
        export_file(
            &file,
            &[AliasRecord {
                name: "gs".into(),
                executable: "first".into(),
                shells: vec![ShellKind::Bash],
                ..Default::default()
            }],
        );
        let preview = import_preview(&root, &file, ConflictStrategy::Ask).unwrap();
        let original_modified = std::fs::metadata(&file).unwrap().modified().unwrap();
        let original = std::fs::read_to_string(&file).unwrap();
        // Same byte length, restored mtime: only the content hash can detect it.
        let changed = original.replace("\"first\"", "\"firsT\"");
        assert_eq!(changed.len(), original.len());
        std::fs::write(&file, changed).unwrap();
        let handle = std::fs::OpenOptions::new().write(true).open(&file).unwrap();
        handle.set_modified(original_modified).unwrap();
        drop(handle);
        let after = file_fingerprint(&file).unwrap();
        assert_eq!(after.len, preview.file_fingerprint.len);
        assert_eq!(
            after.modified_nanos, preview.file_fingerprint.modified_nanos,
            "mtime must be restored for this test to exercise the content hash"
        );
        assert_ne!(after.sha256, preview.file_fingerprint.sha256);
        let error = import_confirm(
            &root,
            &file,
            ConflictStrategy::Ask,
            &BTreeMap::new(),
            &preview.file_fingerprint,
        )
        .unwrap_err();
        assert!(error.contains("changed since the preview"), "{error}");
        assert!(!root.join("aliases.db").exists());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn file_swap_between_fingerprint_check_and_read_is_detected() {
        let root = temp_root("import-mid-read-swap");
        let file = root.join("input.json");
        export_file(
            &file,
            &[AliasRecord {
                name: "gs".into(),
                executable: "first".into(),
                shells: vec![ShellKind::Bash],
                ..Default::default()
            }],
        );
        let expected = file_fingerprint(&file).unwrap();
        let swap_path = file.clone();
        let error = read_checked_import_records(&file, Some(&expected), move || {
            std::fs::write(&swap_path, "[]").unwrap();
        })
        .unwrap_err();
        assert!(error.contains("changed while it was being read"), "{error}");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn file_changed_before_transaction_is_refused_without_creating_the_database() {
        let root = temp_root("import-before-transaction");
        let file = root.join("input.json");
        export_file(
            &file,
            &[AliasRecord {
                name: "gs".into(),
                executable: "first".into(),
                shells: vec![ShellKind::Bash],
                ..Default::default()
            }],
        );
        let preview = import_preview(&root, &file, ConflictStrategy::Ask).unwrap();
        assert!(!root.join("aliases.db").exists());
        let swap_path = file.clone();
        let error = import_confirm_after_read(
            &root,
            &file,
            ConflictStrategy::Ask,
            &BTreeMap::new(),
            &preview.file_fingerprint,
            move || {
                std::fs::write(&swap_path, "[]").unwrap();
            },
        )
        .unwrap_err();
        assert!(error.contains("before the import transaction"), "{error}");
        assert!(!root.join("aliases.db").exists());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn import_confirm_re_evaluates_conflicts_created_after_preview() {
        let root = temp_root("import-recheck");
        let file = root.join("input.json");
        export_file(
            &file,
            &[AliasRecord {
                name: "gs".into(),
                executable: "imported".into(),
                shells: vec![ShellKind::Bash],
                ..Default::default()
            }],
        );
        let preview = import_preview(&root, &file, ConflictStrategy::Ask).unwrap();
        assert_eq!(preview.imported, vec!["gs"]);

        let database = Database::open(root.join("aliases.db")).unwrap();
        insert(
            &database,
            AliasRecord {
                name: "gs".into(),
                executable: "existing".into(),
                shells: vec![ShellKind::Bash],
                ..Default::default()
            },
        );
        drop(database);
        let report = import_confirm(
            &root,
            &file,
            ConflictStrategy::Ask,
            &BTreeMap::new(),
            &file_fingerprint(&file).unwrap(),
        )
        .unwrap();
        assert!(report.imported.is_empty());
        assert_eq!(report.skipped, vec!["gs"]);
        let database = Database::open(root.join("aliases.db")).unwrap();
        assert_eq!(database.list_aliases().unwrap().len(), 1);
        assert_eq!(
            database
                .get_alias_by_name("gs")
                .unwrap()
                .unwrap()
                .executable,
            "existing"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn import_conflict_strategies_produce_distinct_counts_and_results() {
        let root = temp_root("import-conflicts");
        let database = Database::open(root.join("aliases.db")).unwrap();
        insert(
            &database,
            AliasRecord {
                name: "gs".into(),
                executable: "old".into(),
                description: "old".into(),
                shells: vec![ShellKind::Bash],
                ..Default::default()
            },
        );
        drop(database);
        let file = root.join("input.json");
        export_file(
            &file,
            &[AliasRecord {
                name: "gs".into(),
                executable: "new".into(),
                description: "imported".into(),
                shells: vec![ShellKind::Bash],
                ..Default::default()
            }],
        );
        let skip = import_preview(&root, &file, ConflictStrategy::Skip).unwrap();
        assert!(skip.imported.is_empty());
        assert_eq!(skip.skipped, vec!["gs"]);
        let ask = import_preview(&root, &file, ConflictStrategy::Ask).unwrap();
        assert_eq!(ask.skipped, vec!["gs"]);
        let overwrite = import_preview(&root, &file, ConflictStrategy::Overwrite).unwrap();
        assert_eq!(overwrite.imported, vec!["gs"]);
        let renamed = import_preview(&root, &file, ConflictStrategy::Rename).unwrap();
        assert_eq!(renamed.imported, vec!["gs_imported"]);

        import_confirm(
            &root,
            &file,
            ConflictStrategy::Overwrite,
            &overwrite.overwrite_existing,
            &overwrite.file_fingerprint,
        )
        .unwrap();
        let database = Database::open(root.join("aliases.db")).unwrap();
        let alias = database.get_alias_by_name("gs").unwrap().unwrap();
        assert_eq!(alias.executable, "new");
        assert_eq!(alias.description, "imported");

        // Rename confirm inserts the renamed record and keeps the original.
        import_confirm(
            &root,
            &file,
            ConflictStrategy::Rename,
            &BTreeMap::new(),
            &file_fingerprint(&file).unwrap(),
        )
        .unwrap();
        let database = Database::open(root.join("aliases.db")).unwrap();
        assert!(database.get_alias_by_name("gs_imported").unwrap().is_some());
        assert!(database.get_alias_by_name("gs").unwrap().is_some());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn overwrite_skips_powershell_casefold_conflicts_instead_of_partial_writes() {
        let root = temp_root("import-casefold");
        let database = Database::open(root.join("aliases.db")).unwrap();
        insert(
            &database,
            AliasRecord {
                name: "Build".into(),
                executable: "old".into(),
                shells: vec![ShellKind::PowerShell7],
                ..Default::default()
            },
        );
        drop(database);
        let file = root.join("casefold.json");
        export_file(
            &file,
            &[AliasRecord {
                name: "build".into(),
                executable: "new".into(),
                shells: vec![ShellKind::PowerShell7],
                ..Default::default()
            }],
        );
        // Incoming PowerShell record: preview and confirm both skip the
        // case-fold conflict with identical counts/warnings.
        let preview = import_preview(&root, &file, ConflictStrategy::Overwrite).unwrap();
        assert!(preview.imported.is_empty());
        assert_eq!(preview.skipped, vec!["build"]);
        assert!(preview
            .warnings
            .iter()
            .any(|warning| warning.contains("case-fold conflict")));
        let report = import_confirm(
            &root,
            &file,
            ConflictStrategy::Overwrite,
            &preview.overwrite_existing,
            &preview.file_fingerprint,
        )
        .unwrap();
        assert!(report.imported.is_empty());
        assert_eq!(report.skipped, vec!["build"]);
        assert!(report
            .warnings
            .iter()
            .any(|warning| warning.contains("case-fold conflict")));
        let database = Database::open(root.join("aliases.db")).unwrap();
        let names = database
            .list_aliases()
            .unwrap()
            .into_iter()
            .map(|alias| alias.name)
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["Build"]);

        // Incoming Bash record has no PowerShell case-fold rule and may coexist.
        let bash_file = root.join("casefold-bash.json");
        export_file(
            &bash_file,
            &[AliasRecord {
                name: "build".into(),
                executable: "bash-target".into(),
                shells: vec![ShellKind::Bash],
                ..Default::default()
            }],
        );
        let preview = import_preview(&root, &bash_file, ConflictStrategy::Overwrite).unwrap();
        assert_eq!(preview.imported, vec!["build"]);
        assert!(preview.warnings.is_empty());
        let report = import_confirm(
            &root,
            &bash_file,
            ConflictStrategy::Overwrite,
            &preview.overwrite_existing,
            &preview.file_fingerprint,
        )
        .unwrap();
        assert_eq!(report.imported, vec!["build"]);
        let database = Database::open(root.join("aliases.db")).unwrap();
        assert_eq!(
            database
                .get_alias_by_name("build")
                .unwrap()
                .unwrap()
                .executable,
            "bash-target"
        );
        assert!(database.get_alias_by_name("Build").unwrap().is_some());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn sensitive_environment_values_are_filtered_on_import_and_export() {
        let root = temp_root("import-sensitive");
        let file = root.join("input.json");
        let mut alias = AliasRecord {
            name: "secret".into(),
            executable: "tool".into(),
            shells: vec![ShellKind::Bash],
            ..Default::default()
        };
        alias.environment.insert("TOKEN".into(), "hunter2".into());
        alias.environment.insert("MODE".into(), "fast".into());
        write_raw_export(&file, vec![alias], 1);
        let preview = import_preview(&root, &file, ConflictStrategy::Ask).unwrap();
        assert!(preview
            .warnings
            .iter()
            .any(|warning| warning.contains("sensitive environment variable omitted")));
        assert!(!preview.warnings.join("\n").contains("hunter2"));
        import_confirm(
            &root,
            &file,
            ConflictStrategy::Ask,
            &BTreeMap::new(),
            &file_fingerprint(&file).unwrap(),
        )
        .unwrap();
        let database = Database::open(root.join("aliases.db")).unwrap();
        let saved = database.get_alias_by_name("secret").unwrap().unwrap();
        assert!(!saved.environment.contains_key("TOKEN"));
        assert_eq!(
            saved.environment.get("MODE").map(String::as_str),
            Some("fast")
        );

        let exported = root.join("out.json");
        let summary = export_aliases(&root, &exported, false).unwrap();
        assert_eq!(summary.count, 1);
        let raw = std::fs::read_to_string(&exported).unwrap();
        assert!(!raw.contains("hunter2"));
        assert!(!raw.contains("TOKEN"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn empty_import_and_relative_restriction_records_are_handled() {
        let root = temp_root("import-empty");
        let empty = root.join("empty.json");
        std::fs::write(&empty, "[]").unwrap();
        let report = import_preview(&root, &empty, ConflictStrategy::Ask).unwrap();
        assert!(report.imported.is_empty());
        assert!(report.skipped.is_empty());
        assert!(!root.join("aliases.db").exists());

        let relative = root.join("relative.json");
        export_file(
            &relative,
            &[AliasRecord {
                name: "relative".into(),
                executable: "tool".into(),
                shells: vec![ShellKind::Bash],
                path_mode: aliasmgr_core::model::PathMode::RelativeToWorkingDirectory,
                ..Default::default()
            }],
        );
        let report = import_preview(&root, &relative, ConflictStrategy::Ask).unwrap();
        assert!(report.imported.is_empty());
        assert_eq!(report.skipped, vec!["relative"]);
        assert!(report
            .warnings
            .iter()
            .any(|warning| warning.contains("relative path requires")));
        assert!(!root.join("aliases.db").exists());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn invalid_formats_versions_and_missing_files_are_friendly() {
        let root = temp_root("import-invalid");
        let bad_json = root.join("bad.json");
        std::fs::write(&bad_json, "{not json").unwrap();
        let error = import_preview(&root, &bad_json, ConflictStrategy::Ask).unwrap_err();
        assert!(error.contains("invalid JSON export"), "{error}");
        assert!(!root.join("aliases.db").exists());

        let bad_version = root.join("version.json");
        write_raw_export(&bad_version, Vec::new(), 99);
        let error = import_preview(&root, &bad_version, ConflictStrategy::Ask).unwrap_err();
        assert!(error.contains("unsupported format_version"), "{error}");

        let bad_toml = root.join("bad.toml");
        std::fs::write(&bad_toml, "not = [valid").unwrap();
        let error = import_preview(&root, &bad_toml, ConflictStrategy::Ask).unwrap_err();
        assert!(error.contains("invalid TOML export"), "{error}");

        let missing = root.join("missing.json");
        let error = import_preview(&root, &missing, ConflictStrategy::Ask).unwrap_err();
        assert!(error.contains("file not found"), "{error}");

        let directory = root.join("import-dir");
        std::fs::create_dir(&directory).unwrap();
        let error = import_preview(&root, &directory, ConflictStrategy::Ask).unwrap_err();
        assert!(error.contains("path is a directory"), "{error}");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    #[allow(clippy::permissions_set_readonly_false)] // cleanup only
    fn export_handles_formats_overwrite_and_missing_parent_friendly() {
        let root = temp_root("export-errors");
        let database = Database::open(root.join("aliases.db")).unwrap();
        insert(
            &database,
            AliasRecord {
                name: "gs".into(),
                executable: "git".into(),
                shells: vec![ShellKind::Bash],
                ..Default::default()
            },
        );
        drop(database);
        let json = root.join("out.json");
        let summary = export_aliases(&root, &json, false).unwrap();
        assert_eq!(summary.count, 1);
        assert_eq!(summary.format, "JSON");
        let error = export_aliases(&root, &json, false).unwrap_err();
        assert!(error.contains("already exists"), "{error}");
        export_aliases(&root, &json, true).unwrap();

        let toml = root.join("out.toml");
        let summary = export_aliases(&root, &toml, false).unwrap();
        assert_eq!(summary.format, "TOML");
        assert!(std::fs::read_to_string(&toml)
            .unwrap()
            .contains("format_version"));

        let missing = root.join("missing-dir").join("out.json");
        let error = export_aliases(&root, &missing, false).unwrap_err();
        assert!(error.contains("target directory does not exist"), "{error}");

        let directory = root.join("target-dir");
        std::fs::create_dir(&directory).unwrap();
        let error = export_aliases(&root, &directory, true).unwrap_err();
        assert!(error.contains("target is a directory"), "{error}");
        assert!(!error.starts_with("os error"), "{error}");

        // A read-only target must keep its previous content and leave no temp.
        let readonly = root.join("readonly.json");
        std::fs::write(&readonly, "previous").unwrap();
        let mut permissions = std::fs::metadata(&readonly).unwrap().permissions();
        permissions.set_readonly(true);
        std::fs::set_permissions(&readonly, permissions.clone()).unwrap();
        let error = export_aliases(&root, &readonly, true).unwrap_err();
        assert!(error.contains("permission denied"), "{error}");
        assert_eq!(std::fs::read_to_string(&readonly).unwrap(), "previous");
        let leftovers = std::fs::read_dir(&root)
            .unwrap()
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_name().to_string_lossy().contains(".tmp."))
            .count();
        assert_eq!(leftovers, 0);
        permissions.set_readonly(false);
        let _ = std::fs::set_permissions(&readonly, permissions);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn large_import_preview_and_confirm_keep_counts() {
        let root = temp_root("import-large");
        let file = root.join("large.json");
        let aliases = (0..300)
            .map(|index| AliasRecord {
                name: format!("bulk_{index:03}"),
                executable: "tool".into(),
                shells: vec![ShellKind::Bash],
                ..Default::default()
            })
            .collect::<Vec<_>>();
        write_raw_export(&file, aliases, 1);
        let preview = import_preview(&root, &file, ConflictStrategy::Ask).unwrap();
        assert_eq!(preview.imported.len(), 300);
        import_confirm(
            &root,
            &file,
            ConflictStrategy::Ask,
            &BTreeMap::new(),
            &file_fingerprint(&file).unwrap(),
        )
        .unwrap();
        let database = Database::open(root.join("aliases.db")).unwrap();
        assert_eq!(database.list_aliases().unwrap().len(), 300);
        let _ = std::fs::remove_dir_all(root);
    }
}
