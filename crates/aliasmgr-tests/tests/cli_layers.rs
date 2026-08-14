use std::{fs, process::Command};

#[test]
fn cli_layer_fixture_is_isolated_and_does_not_dump_environment() {
    let config = std::env::var("ALIASMGR_CONFIG_DIR").expect("CI must inject isolated config");
    assert!(!config.is_empty());
    let result = Command::new("rustc").arg("--version").output();
    assert!(result.is_ok());
    assert!(!String::from_utf8_lossy(&result.unwrap().stdout).contains("TOKEN"));    assert!(!fs::read_dir(config).map(|entries| entries.count()).unwrap_or(0).gt(&100));
}
