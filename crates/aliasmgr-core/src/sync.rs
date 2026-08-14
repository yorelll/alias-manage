use crate::{error::AliasError, lock::FileLock, model::{AliasRecord, ShellKind, ShellStatus, ShellSyncResult, SyncReceipt}, shells::{bash, powershell, zsh}};
use std::{fs, path::PathBuf, time::Duration};

pub struct SyncCoordinator { pub config_dir: PathBuf }

pub fn shell_status(applied_revision: i64, current_revision: i64, loader_installed: bool, error: Option<&str>) -> ShellStatus {
    if error.is_some() { ShellStatus::Failed }
    else if !loader_installed { ShellStatus::LoaderMissing }
    else if applied_revision == current_revision { ShellStatus::Ok }
    else if applied_revision < current_revision { ShellStatus::Stale }
    else { ShellStatus::Unknown }
}

impl SyncCoordinator {
    pub fn new(config_dir: impl Into<PathBuf>) -> Self { Self { config_dir: config_dir.into() } }

    pub fn apply(&self, aliases: &[AliasRecord], shells: &[ShellKind], revision: i64) -> Result<SyncReceipt, AliasError> {
        fs::create_dir_all(self.config_dir.join("generated"))?;
        let _lock = FileLock::acquire(self.config_dir.join("sync.lock"), Duration::from_secs(10))?;
        let journal = self.config_dir.join("operation.journal");
        fs::write(&journal, format!("revision_from={}\nrevision_to={}\nstate=prepared\nbackups=[]\n", revision.saturating_sub(1), revision))?;
        let mut results = Vec::new();
        for shell in shells {
            let path = self.generated_path(shell);
            let temp = path.with_extension(format!("tmp.{}", std::process::id()));
            match self.render_shell(aliases, shell, revision) {
                Ok(body) => {
                    fs::write(&temp, &body)?;
                    fs::rename(&temp, &path)?;
                    results.push(ShellSyncResult { shell: shell.clone(), status: shell_status(revision, revision, true, None), error: None });
                }
                Err(error) => {
                    let message = error.to_string();
                    let _ = fs::remove_file(&temp);
                    results.push(ShellSyncResult { shell: shell.clone(), status: shell_status(revision.saturating_sub(1), revision, true, Some(&message)), error: Some(message) });
                }
            }
        }
        fs::write(&journal, format!("revision_from={}\nrevision_to={}\nstate=completed\nbackups=[]\n", revision.saturating_sub(1), revision))?;
        Ok(SyncReceipt { revision, results })
    }

    pub fn recover_pending_operations(&self) -> Result<(), AliasError> {
        let journal = self.config_dir.join("operation.journal");
        if journal.exists() { let content = fs::read_to_string(&journal)?; if content.contains("state=prepared") { fs::remove_file(journal)?; } }
        Ok(())
    }

    fn generated_path(&self, shell: &ShellKind) -> PathBuf {
        let name = match shell { ShellKind::Bash => "bash.sh", ShellKind::Zsh => "zsh.sh", ShellKind::PowerShell5 => "powershell5.ps1", ShellKind::PowerShell7 => "powershell7.ps1", _ => "unsupported.sh" };
        self.config_dir.join("generated").join(name)
    }

    fn render_shell(&self, aliases: &[AliasRecord], shell: &ShellKind, revision: i64) -> Result<String, AliasError> {
        let mut output = format!("# Alias Manager\n# revision: {revision}\n# managed: {}\n", aliases.iter().map(|a| a.name.as_str()).collect::<Vec<_>>().join(" "));
        let names = crate::model::ManagedNameSet { current: aliases.iter().map(|a| a.name.clone()).collect(), retired: Vec::new() };
        for alias in aliases.iter().filter(|alias| alias.enabled) { output.push_str(&match shell { ShellKind::Bash => bash::render(alias, &names)?, ShellKind::Zsh => zsh::render(alias, &names)?, ShellKind::PowerShell5 | ShellKind::PowerShell7 => powershell::render(alias, shell.clone(), &names)?, _ => return Err(AliasError::ShellNotInstalled) }); }
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::TargetType;
    #[test]
    fn computes_shell_states() {
        assert_eq!(shell_status(2, 2, true, None), ShellStatus::Ok);
        assert_eq!(shell_status(1, 2, true, None), ShellStatus::Stale);
        assert_eq!(shell_status(2, 2, false, None), ShellStatus::LoaderMissing);
        assert_eq!(shell_status(1, 2, true, Some("syntax")), ShellStatus::Failed);
    }
    #[test]
    fn applies_each_shell_and_writes_journal() {
        let root = std::env::temp_dir().join(format!("aliasmgr-sync-{}", std::process::id()));
        let alias = AliasRecord { name: "gs".into(), executable: "git".into(), target_type: TargetType::NativeExecutable, ..Default::default() };
        let receipt = SyncCoordinator::new(&root).apply(&[alias], &[ShellKind::Bash, ShellKind::Zsh], 2).unwrap();
        assert_eq!(receipt.results.len(), 2); assert!(root.join("operation.journal").exists()); assert!(root.join("generated/bash.sh").exists());
        let _ = fs::remove_dir_all(root);
    }
    #[test]
    fn partial_shell_failure_keeps_successful_shell_result() {
        let root = std::env::temp_dir().join(format!("aliasmgr-partial-{}", std::process::id()));
        let alias = AliasRecord { name: "bad".into(), executable: "tool".into(), advanced_shell_mode: true, ..Default::default() };
        let receipt = SyncCoordinator::new(&root).apply(&[alias], &[ShellKind::Bash, ShellKind::PowerShell7], 2).unwrap();
        assert_eq!(receipt.results.len(), 2);
        assert!(receipt.results.iter().any(|result| result.status == ShellStatus::Failed));
        let _ = fs::remove_dir_all(root);
    }
    #[test]
    fn recovery_removes_prepared_journal() {
        let root = std::env::temp_dir().join(format!("aliasmgr-recovery-{}", std::process::id())); fs::create_dir_all(&root).unwrap();
        fs::write(root.join("operation.journal"), "state=prepared\n").unwrap();
        SyncCoordinator::new(&root).recover_pending_operations().unwrap(); assert!(!root.join("operation.journal").exists()); let _ = fs::remove_dir_all(root);
    }
}
