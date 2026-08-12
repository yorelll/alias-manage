use crate::error::AliasError;
use fs4::fs_std::FileExt;
use std::{fs::{self, File, OpenOptions}, path::{Path, PathBuf}, thread, time::{Duration, Instant}};

pub struct FileLock { file: File, path: PathBuf }

impl FileLock {
    pub fn acquire(path: impl AsRef<Path>, timeout: Duration) -> Result<Self, AliasError> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() { fs::create_dir_all(parent)?; }
        let file = OpenOptions::new().create(true).read(true).write(true).open(&path)?;
        let started = Instant::now();
        loop {
            match file.try_lock_exclusive() {
                Ok(true) => {
                    file.set_len(0)?;
                    let owner = format!("pid={}\nstarted_at={}\n", std::process::id(), chrono::Utc::now().to_rfc3339());
                    std::io::Write::write_all(&mut &file, owner.as_bytes())?;
                    return Ok(Self { file, path });
                }
                Err(_) if started.elapsed() >= timeout => return Err(AliasError::LockTimeout),
                Err(_) => thread::sleep(Duration::from_millis(25)),
            }
        }
    }
}

impl Drop for FileLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
        let _ = fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lock_writes_owner_and_second_lock_times_out() {
        let path = std::env::temp_dir().join(format!("aliasmgr-lock-{}.lock", std::process::id()));
        let first = FileLock::acquire(&path, Duration::from_millis(100)).unwrap();
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("pid="));
        assert!(matches!(FileLock::acquire(&path, Duration::from_millis(25)), Err(AliasError::LockTimeout)));
        drop(first);
        assert!(!path.exists());
    }
}
