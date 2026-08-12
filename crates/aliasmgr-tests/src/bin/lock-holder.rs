use aliasmgr_core::lock::FileLock;
use std::{env, path::PathBuf, time::Duration};

fn main() {
    let path = PathBuf::from(env::args().nth(1).expect("lock path is required"));
    let _lock = FileLock::acquire(path, Duration::from_secs(5)).expect("lock acquisition failed");
    std::thread::sleep(Duration::from_millis(500));
}
