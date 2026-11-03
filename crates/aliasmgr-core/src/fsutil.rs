//! Small filesystem primitives shared by configuration and export writers.

use crate::error::AliasError;
use std::{
    fs::{self, OpenOptions},
    io::{Error, ErrorKind, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Follows symlink components to the final target so an atomic replacement
/// writes through a dotfile-style symlink instead of replacing the link itself.
fn resolve_target(path: &Path) -> PathBuf {
    let mut current = path.to_path_buf();
    for _ in 0..16 {
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => match fs::read_link(&current) {
                Ok(link) => {
                    current = if link.is_absolute() {
                        link
                    } else {
                        current
                            .parent()
                            .unwrap_or_else(|| Path::new("."))
                            .join(link)
                    };
                }
                Err(_) => break,
            },
            _ => break,
        }
    }
    current
}

fn temp_candidate(parent: &Path, name: &str) -> PathBuf {
    let nonce = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or_default();
    parent.join(format!(
        ".{name}.tmp.{}.{nonce}.{stamp}",
        std::process::id()
    ))
}

/// Writes `content` to `path` atomically: a unique temporary file in the same
/// directory is created with restrictive permissions, given the previous
/// target's permissions, and renamed over the target. A failed write or rename
/// removes the temporary file and leaves the previous target intact.
///
/// Symlinked targets are resolved and written through. On Unix the temp file is
/// created with mode 0600 and then receives the existing target's mode. Windows
/// ACLs are not copied with std APIs: the temp file inherits the directory ACL,
/// while the read-only attribute is preserved.
pub fn write_atomic(path: &Path, content: &str) -> Result<(), AliasError> {
    let target = resolve_target(path);
    let existing_permissions = fs::metadata(&target)
        .ok()
        .map(|metadata| metadata.permissions());
    if let Some(permissions) = &existing_permissions {
        if permissions.readonly() {
            return Err(AliasError::Io(Error::new(
                ErrorKind::PermissionDenied,
                format!("target is read-only: {}", target.display()),
            )));
        }
    }
    let parent = target
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let name = target
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "target".to_string());

    let (temp, mut file) = {
        let mut opened = None;
        for _ in 0..64 {
            let candidate = temp_candidate(parent, &name);
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            match options.open(&candidate) {
                Ok(file) => {
                    opened = Some((candidate, file));
                    break;
                }
                Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(AliasError::Io(error)),
            }
        }
        opened.ok_or_else(|| {
            AliasError::Io(Error::new(
                ErrorKind::AlreadyExists,
                format!(
                    "could not create a temporary file next to {}",
                    target.display()
                ),
            ))
        })?
    };

    let result = (|| -> Result<(), AliasError> {
        file.write_all(content.as_bytes())?;
        file.sync_all()?;
        drop(file);
        if let Some(permissions) = existing_permissions {
            fs::set_permissions(&temp, permissions)?;
        }
        fs::rename(&temp, &target)?;
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

    fn temp_leftovers(root: &Path) -> usize {
        fs::read_dir(root)
            .unwrap()
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_name().to_string_lossy().contains(".tmp."))
            .count()
    }

    #[test]
    fn atomic_write_replaces_an_existing_file_without_leftovers() {
        let root = temp_dir("replace");
        let path = root.join("out.txt");
        fs::write(&path, "old").unwrap();
        write_atomic(&path, "new").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "new");
        assert_eq!(temp_leftovers(&root), 0);
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
        assert_eq!(temp_leftovers(&root), 0);
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

    #[cfg(unix)]
    #[test]
    fn atomic_write_preserves_an_existing_unix_mode() {
        use std::os::unix::fs::PermissionsExt;
        let root = temp_dir("mode");
        let target = root.join("target");
        fs::write(&target, "old").unwrap();
        fs::set_permissions(&target, fs::Permissions::from_mode(0o600)).unwrap();
        write_atomic(&target, "new").unwrap();
        assert_eq!(
            fs::metadata(&target).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(fs::read_to_string(&target).unwrap(), "new");
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn atomic_write_follows_a_symlinked_target_instead_of_replacing_it() {
        let root = temp_dir("symlink");
        let real = root.join("real.txt");
        let link = root.join("link.txt");
        fs::write(&real, "old").unwrap();
        std::os::unix::fs::symlink(&real, &link).unwrap();
        write_atomic(&link, "new").unwrap();
        assert!(fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(fs::read_to_string(&real).unwrap(), "new");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn concurrent_writers_only_produce_complete_files() {
        let root = temp_dir("concurrent");
        let target = root.join("target.txt");
        fs::write(&target, "initial").unwrap();
        let mut threads = Vec::new();
        for index in 0..8 {
            let target = target.clone();
            threads.push(std::thread::spawn(move || {
                for round in 0..20 {
                    write_atomic(&target, &format!("value-{index}-{round}")).unwrap();
                }
            }));
        }
        for thread in threads {
            thread.join().unwrap();
        }
        let content = fs::read_to_string(&target).unwrap();
        assert!(content.starts_with("value-"), "{content}");
        assert_eq!(temp_leftovers(&root), 0);
        let _ = fs::remove_dir_all(root);
    }
}
