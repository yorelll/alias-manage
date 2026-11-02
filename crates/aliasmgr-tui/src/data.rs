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
    integration::{self, LoaderOutcome},
    model::{AliasRecord, ManagedNameSet, ShellKind, ShellState, ShellStatus},
    storage::Database,
    sync::{self, ShellContentState, ShellContentStatus, SyncRun},
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
    Doctor,
    LoaderInstall(String),
    LoaderUninstall(String),
}

impl JobKind {
    pub fn label(&self) -> String {
        match self {
            JobKind::Sync => "syncing".to_string(),
            JobKind::Doctor => "running doctor".to_string(),
            JobKind::LoaderInstall(shell) => format!("installing {shell} loader"),
            JobKind::LoaderUninstall(shell) => format!("removing {shell} loader"),
        }
    }
}

#[derive(Debug)]
pub enum JobResult {
    Sync(Result<SyncRun, String>),
    Doctor(Result<Vec<DoctorFinding>, String>),
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
        JobKind::Doctor => JobResult::Doctor(run_doctor(root)),
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
}
