//! Small filesystem primitives shared by configuration and export writers.

use crate::error::AliasError;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Error, ErrorKind, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// SHA-256 of an in-memory buffer.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

/// SHA-256 of a file's bytes, used for strong "same previewed content" checks.
pub fn file_sha256(path: &Path) -> Result<String, AliasError> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher)?;
    Ok(format!("{:x}", hasher.finalize()))
}

/// Follows symlink components to the final target so an atomic replacement
/// writes through a dotfile-style symlink instead of replacing the link itself.
/// A cyclic or excessively long chain is an error; a link is never replaced.
fn resolve_target(path: &Path) -> Result<PathBuf, AliasError> {
    let mut current = path.to_path_buf();
    for _ in 0..64 {
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
                Err(error) => return Err(AliasError::Io(error)),
            },
            _ => return Ok(current),
        }
    }
    // The 64th hop may itself have landed on a real file; only reject when the
    // final path is still a link (a longer chain or a cycle).
    match fs::symlink_metadata(&current) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(AliasError::Io(Error::new(
            ErrorKind::InvalidInput,
            format!("symlink chain is cyclic or too deep: {}", path.display()),
        ))),
        _ => Ok(current),
    }
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

#[cfg(unix)]
fn set_unix_mode(options: &mut OpenOptions, mode: u32) {
    use std::os::unix::fs::OpenOptionsExt;
    options.mode(mode);
}

#[cfg(not(unix))]
fn set_unix_mode(_options: &mut OpenOptions, _mode: u32) {}

/// The temp file must never be more permissive than the file it replaces: an
/// existing target's mode wins, otherwise the caller's new-file mode applies
/// (and the process umask still restricts it).
#[cfg(unix)]
fn temp_creation_mode(existing: Option<&fs::Permissions>, new_file_mode: u32) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    existing
        .map(|permissions| permissions.mode() & 0o777)
        .unwrap_or(new_file_mode)
}

#[cfg(not(unix))]
fn temp_creation_mode(_existing: Option<&fs::Permissions>, new_file_mode: u32) -> u32 {
    new_file_mode
}

fn open_temp(parent: &Path, name: &str, mode: u32) -> Result<(PathBuf, File), AliasError> {
    for _ in 0..64 {
        let candidate = temp_candidate(parent, name);
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        set_unix_mode(&mut options, mode);
        match options.open(&candidate) {
            Ok(file) => return Ok((candidate, file)),
            Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(AliasError::Io(error)),
        }
    }
    Err(AliasError::Io(Error::new(
        ErrorKind::AlreadyExists,
        format!(
            "could not create a temporary file next to {}",
            parent.display()
        ),
    )))
}

/// Writes `content` to `path` atomically: a unique temporary file in the same
/// directory is created with restrictive permissions, given the previous
/// target's permissions, and renamed over the target. A failed write or rename
/// removes the temporary file and leaves the previous target intact.
///
/// Symlinked targets are resolved and written through. On Unix the temp file is
/// created with the existing target's mode (0600 for a new target) and then
/// receives that mode explicitly, so it is never more permissive than the
/// target; a new file keeps 0600. Windows ACLs are not copied with std APIs:
/// the temp file inherits the directory ACL, while the read-only attribute is
/// preserved.
pub fn write_atomic(path: &Path, content: &str) -> Result<(), AliasError> {
    write_atomic_with_mode(path, content, 0o600)
}

/// Like [`write_atomic`] but chooses the Unix mode for a **new** target (the
/// process umask still applies). Existing targets always keep their mode, and
/// the temp file is created with that mode so replacement never exposes a
/// broader window than the file it replaces. Used by exports to preserve the
/// usual umask-derived mode instead of forcing 0600 on newly created export
/// files.
pub fn write_atomic_with_mode(
    path: &Path,
    content: &str,
    new_file_mode: u32,
) -> Result<(), AliasError> {
    let target = resolve_target(path)?;
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
    let temp_mode = temp_creation_mode(existing_permissions.as_ref(), new_file_mode);
    let (temp, mut file) = open_temp(parent, &name, temp_mode)?;

    let result = (|| -> Result<(), AliasError> {
        file.write_all(content.as_bytes())?;
        file.sync_all()?;
        drop(file);
        if let Some(permissions) = existing_permissions {
            fs::set_permissions(&temp, permissions)?;
        }
        fs::rename(&temp, &target)?;
        #[cfg(unix)]
        {
            // Make the rename itself durable where the platform supports it.
            if let Ok(directory) = File::open(parent) {
                let _ = directory.sync_all();
            }
        }
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
    use std::collections::HashSet;
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

    /// Mode a file created with `mode` actually receives under this process's
    /// umask, so tests do not assume a permissive environment.
    #[cfg(unix)]
    fn probe_mode(root: &Path, name: &str, mode: u32) -> u32 {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        let path = root.join(name);
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(mode)
            .open(&path)
            .unwrap();
        let actual = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        fs::remove_file(path).unwrap();
        actual
    }

    #[test]
    fn file_hash_detects_same_size_content_changes() {
        let root = temp_dir("hash");
        let path = root.join("data.txt");
        fs::write(&path, "first").unwrap();
        let first = file_sha256(&path).unwrap();
        fs::write(&path, "firsT").unwrap();
        assert_eq!(fs::metadata(&path).unwrap().len(), 5);
        assert_ne!(first, file_sha256(&path).unwrap());
        let _ = fs::remove_dir_all(root);
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
        // This is the friendly-error path; atomicity is covered below.
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
        // Friendly-error path; previous bytes and temp cleanup are atomic.
        let root = temp_dir("readonly");
        let target = root.join("target");
        fs::write(&target, "original").unwrap();
        let mut permissions = fs::metadata(&target).unwrap().permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&target, permissions.clone()).unwrap();
        let result = write_atomic(&target, "replacement");
        assert!(result.is_err());
        assert_eq!(fs::read_to_string(&target).unwrap(), "original");
        assert_eq!(temp_leftovers(&root), 0);
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
    fn new_private_file_is_created_with_mode_0600() {
        use std::os::unix::fs::PermissionsExt;
        let root = temp_dir("new-mode");
        let expected = probe_mode(&root, "probe", 0o600);
        let target = root.join("config.toml");
        write_atomic(&target, "content").unwrap();
        let actual = fs::metadata(&target).unwrap().permissions().mode() & 0o777;
        assert_eq!(actual, expected, "new config file must use the 0600 mode");
        assert_eq!(actual & 0o077, 0, "new config file must stay private");
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn replacing_a_0600_export_creates_its_temp_with_at_most_0600() {
        use std::os::unix::fs::PermissionsExt;
        let root = temp_dir("temp-mode-cap");
        let target = root.join("export.json");
        fs::write(&target, "old").unwrap();
        fs::set_permissions(&target, fs::Permissions::from_mode(0o600)).unwrap();
        let existing = fs::metadata(&target).unwrap().permissions();
        assert_eq!(temp_creation_mode(Some(&existing), 0o666), 0o600);

        let expected = probe_mode(&root, "probe", 0o600);
        let mode = temp_creation_mode(Some(&existing), 0o666);
        let (temp, file) = open_temp(&root, "export.json", mode).unwrap();
        drop(file);
        let created = fs::metadata(&temp).unwrap().permissions().mode() & 0o777;
        assert_eq!(
            created, expected,
            "temp must be 0600, not the 0644 export default"
        );
        assert_eq!(created & 0o077, 0);
        fs::remove_file(temp).unwrap();

        write_atomic_with_mode(&target, "new", 0o666).unwrap();
        assert_eq!(
            fs::metadata(&target).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(fs::read_to_string(&target).unwrap(), "new");
        assert_eq!(temp_leftovers(&root), 0);
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

    #[cfg(unix)]
    #[test]
    fn cyclic_symlinks_are_refused_without_replacing_a_link() {
        let root = temp_dir("symlink-cycle");
        let first = root.join("first");
        let second = root.join("second");
        std::os::unix::fs::symlink(&second, &first).unwrap();
        std::os::unix::fs::symlink(&first, &second).unwrap();
        let result = write_atomic(&first, "new");
        assert!(
            matches!(&result, Err(AliasError::Io(error)) if error.kind() == ErrorKind::InvalidInput),
            "expected an I/O filesystem-loop error, got {result:?}"
        );
        assert!(fs::symlink_metadata(&first)
            .unwrap()
            .file_type()
            .is_symlink());
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(windows)]
    #[test]
    fn windows_file_symlink_is_written_through_when_supported() {
        use std::os::windows::fs::symlink_file;
        let root = temp_dir("win-symlink");
        let real = root.join("real.txt");
        let link = root.join("link.txt");
        fs::write(&real, "old").unwrap();
        if let Err(error) = symlink_file(&real, &link) {
            if std::env::var_os("ALIASMGR_REQUIRE_SYMLINK_TESTS").is_some() {
                panic!(
                    "Windows symlink creation failed but ALIASMGR_REQUIRE_SYMLINK_TESTS is set: {error}"
                );
            }
            eprintln!(
                concat!(
                    "SKIP windows_file_symlink_is_written_through_when_supported: ",
                    "symlink creation denied: {} ",
                    "(set ALIASMGR_REQUIRE_SYMLINK_TESTS=1 to make this fatal)"
                ),
                error
            );
            let _ = fs::remove_dir_all(root);
            return;
        }
        write_atomic(&link, "new").unwrap();
        assert!(fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(fs::read_to_string(&real).unwrap(), "new");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn temp_candidates_are_unique_across_rapid_calls() {
        let root = temp_dir("candidates");
        let mut seen = HashSet::new();
        for _ in 0..1000 {
            let candidate = temp_candidate(&root, "target");
            assert!(seen.insert(candidate), "duplicate temp candidate");
        }
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn concurrent_writers_only_produce_complete_files() {
        // Smoke test: many writers, complete final content and no leftovers.
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
