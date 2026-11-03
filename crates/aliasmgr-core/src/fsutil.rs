//! Small filesystem primitives shared by configuration and export writers.

use crate::error::AliasError;
use std::{
    fs,
    io::{Error, ErrorKind},
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

/// Writes `content` to `path` atomically: a unique temporary file in the same
/// directory is written first and renamed over the target. A failed write or
/// rename removes the temporary file and leaves the previous target intact.
///
/// A target that is explicitly read-only is refused before any temporary file
/// is created (atomic rename could otherwise replace it on Unix).
pub fn write_atomic(path: &Path, content: &str) -> Result<(), AliasError> {
    if let Ok(metadata) = fs::metadata(path) {
        if metadata.permissions().readonly() {
            return Err(AliasError::Io(Error::new(
                ErrorKind::PermissionDenied,
                format!("target is read-only: {}", path.display()),
            )));
        }
    }
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "target".to_string());
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or_default();
    let temp = parent.join(format!(".{name}.tmp.{}.{nonce}", std::process::id()));
    let result = (|| -> Result<(), AliasError> {
        fs::write(&temp, content)?;
        fs::rename(&temp, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn temp_dir(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "aliasmgr-fsutil-{label}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or_default()
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn atomic_write_replaces_an_existing_file_without_leftovers() {
        let root = temp_dir("replace");
        let path = root.join("out.txt");
        fs::write(&path, "old").unwrap();
        write_atomic(&path, "new").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "new");
        let leftovers = fs::read_dir(&root)
            .unwrap()
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_name().to_string_lossy().contains(".tmp."))
            .count();
        assert_eq!(leftovers, 0);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn directory_target_fails_and_leaves_no_temporary_file() {
        let root = temp_dir("directory");
        let target = root.join("target");
        fs::create_dir(&target).unwrap();
        let result = write_atomic(&target, "content");
        assert!(result.is_err());
        assert!(target.is_dir());
        let leftovers = fs::read_dir(&root)
            .unwrap()
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_name().to_string_lossy().contains(".tmp."))
            .count();
        assert_eq!(leftovers, 0);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[allow(clippy::permissions_set_readonly_false)] // cleanup only
    fn read_only_target_is_preserved() {
        let root = temp_dir("readonly");
        let target = root.join("target");
        fs::write(&target, "original").unwrap();
        let mut permissions = fs::metadata(&target).unwrap().permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&target, permissions.clone()).unwrap();
        let result = write_atomic(&target, "replacement");
        assert!(result.is_err());
        assert_eq!(fs::read_to_string(&target).unwrap(), "original");
        permissions.set_readonly(false);
        let _ = fs::set_permissions(&target, permissions);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[allow(clippy::permissions_set_readonly_false)] // cleanup only
    fn unwritable_parent_leaves_the_target_intact() {
        let root = temp_dir("parent");
        let target = root.join("target.txt");
        fs::write(&target, "original").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = fs::metadata(&root).unwrap().permissions();
            permissions.set_mode(0o555);
            fs::set_permissions(&root, permissions.clone()).unwrap();
            let result = write_atomic(&target, "replacement");
            assert!(result.is_err());
            assert_eq!(fs::read_to_string(&target).unwrap(), "original");
            let mut restore = fs::metadata(&root).unwrap().permissions();
            restore.set_mode(0o755);
            fs::set_permissions(&root, restore).unwrap();
        }
        let _ = fs::remove_dir_all(root);
    }
}
