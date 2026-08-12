use aliasmgr_core::{error::AliasError, lock::FileLock};
use std::time::Duration;

#[test]
fn cross_crate_lock_smoke_test() {
    let path = std::env::temp_dir().join(format!("aliasmgr-cross-lock-{}.lock", std::process::id()));
    let first = FileLock::acquire(&path, Duration::from_millis(100)).unwrap();
    assert!(matches!(FileLock::acquire(&path, Duration::from_millis(25)), Err(AliasError::LockTimeout)));
    drop(first);
}
