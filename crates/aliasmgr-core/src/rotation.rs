use crate::error::AliasError;
use std::{fs, path::{Path, PathBuf}};

pub fn prune_oldest(directory: &Path, keep: usize, protected: &[PathBuf]) -> Result<(), AliasError> {
    let mut files: Vec<_> = fs::read_dir(directory)?.filter_map(Result::ok).filter(|entry| entry.path().is_file() && !protected.contains(&entry.path())).collect();
    files.sort_by_key(|entry| entry.metadata().and_then(|metadata| metadata.modified()).ok());
    while files.len() > keep { if let Some(entry) = files.remove(0) { fs::remove_file(entry.path())?; } }
    Ok(())
}

pub fn path_is_other_writable(path: &Path) -> Result<bool, AliasError> {
    let metadata = fs::metadata(path)?;
    #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; Ok(metadata.permissions().mode() & 0o002 != 0) }
    #[cfg(windows)] { let _ = metadata; Ok(false) }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rotation_preserves_protected_file() {
        let root = std::env::temp_dir().join(format!("aliasmgr-rotation-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let old = root.join("old"); let current = root.join("current");
        fs::write(&old, "old").unwrap(); fs::write(&current, "current").unwrap();
        prune_oldest(&root, 1, std::slice::from_ref(&current)).unwrap();
        assert!(current.exists()); assert!(!old.exists()); let _ = fs::remove_dir_all(root);
    }
}
