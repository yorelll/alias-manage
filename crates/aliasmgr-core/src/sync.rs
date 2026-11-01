use crate::{
    config::{AppConfig, AppPaths},
    error::AliasError,
    lock::FileLock,
    model::{AliasRecord, ManagedNameSet, ShellKind, ShellStatus, ShellSyncResult, SyncReceipt},
    shells::{bash, powershell, zsh},
};
use sha2::{Digest, Sha256};
use std::{
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
                        loader_installed: true,
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
                        loader_installed: true,
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
        for alias in aliases.iter().filter(|alias| alias.enabled) {
            output.push_str(&match shell {
                ShellKind::Bash => bash::render(alias, names)?,
                ShellKind::Zsh => zsh::render(alias, names)?,
                ShellKind::PowerShell5 | ShellKind::PowerShell7 => {
                    powershell::render(alias, shell.clone(), names)?
                }
                _ => return Err(AliasError::ShellNotInstalled),
            });
        }
        Ok(output)
    }
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
}
