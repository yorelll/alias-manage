use crate::error::AliasError;
use std::{fs, path::{Path, PathBuf}};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UninstallMode { RetainAliases, PurgeAliases }
#[derive(Debug, Default)]
pub struct UninstallReport { pub removed_files: Vec<PathBuf>, pub retained_targets: Vec<PathBuf>, pub loader_removed: bool }

pub fn uninstall(config_dir: &Path, mode: UninstallMode, loader_path: Option<&Path>) -> Result<UninstallReport, AliasError> {
    let mut report = UninstallReport::default();
    if let Some(loader) = loader_path { if loader.exists() { let content = fs::read_to_string(loader)?; let cleaned = crate::shells::common::remove_loader(&content)?; fs::write(loader, cleaned)?; report.loader_removed = true; } }
    if mode == UninstallMode::PurgeAliases {
        for relative in ["generated/bash.sh", "generated/zsh.sh", "generated/powershell5.ps1", "generated/powershell7.ps1", "aliases.db", "operation.journal"] {
            let path = config_dir.join(relative); if path.exists() { fs::remove_file(&path)?; report.removed_files.push(path); }
        }
    }
    Ok(report)
}

pub fn guard_missing_generated(path: &Path) -> bool { !path.exists() }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retain_mode_removes_loader_but_keeps_generated_targets() {
        let root = std::env::temp_dir().join(format!("aliasmgr-uninstall-{}", std::process::id())); fs::create_dir_all(root.join("generated")).unwrap();
        fs::write(root.join("generated/bash.sh"), "command target\n").unwrap();
        let rc = root.join(".bashrc"); fs::write(&rc, "before\n# >>> Alias Manager >>>\nmanaged\n# <<< Alias Manager <<<\nafter\n").unwrap();
        let report = uninstall(&root, UninstallMode::RetainAliases, Some(&rc)).unwrap();
        assert!(report.loader_removed); assert!(root.join("generated/bash.sh").exists()); assert!(rc.exists()); let _ = fs::remove_dir_all(root);
    }
    #[test]
    fn purge_mode_removes_managed_files_but_never_target_file() {
        let root = std::env::temp_dir().join(format!("aliasmgr-purge-{}", std::process::id())); fs::create_dir_all(root.join("generated")).unwrap();
        let target = root.join("target.py"); fs::write(&target, "print(1)\n").unwrap(); fs::write(root.join("aliases.db"), "db").unwrap();
        uninstall(&root, UninstallMode::PurgeAliases, None).unwrap(); assert!(target.exists()); assert!(!root.join("aliases.db").exists()); let _ = fs::remove_dir_all(root);
    }
}
