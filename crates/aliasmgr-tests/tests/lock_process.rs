use aliasmgr_core::{error::AliasError, lock::FileLock};
use std::{path::PathBuf, process::Command, time::Duration};

#[test]
fn independent_process_holds_lock() {
    let path = std::env::temp_dir().join(format!("aliasmgr-process-lock-{}.lock", std::process::id()));
    let binary = env!("CARGO_BIN_EXE_lock-holder");
    let mut child = Command::new(binary).arg(&path).spawn().unwrap();
    std::thread::sleep(Duration::from_millis(100));
    assert!(matches!(FileLock::acquire(&path, Duration::from_millis(100)), Err(AliasError::LockTimeout)));
    child.kill().unwrap();
    let _ = child.wait();
    let _ = std::fs::remove_file(PathBuf::from(path));
}
