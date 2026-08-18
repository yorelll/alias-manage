use aliasmgr_cli::commands;

#[test]
fn doctor_reports_missing_configuration_and_reload_print_is_stable() {
    let root = std::env::temp_dir().join(format!("aliasmgr-doctor-{}", std::process::id()));
    let root_string = root.to_string_lossy().to_string();
    let findings = commands::doctor(Some(&root_string)).unwrap();
    assert!(findings.iter().any(|finding| finding.contains("数据库")));
    assert!(commands::reload_print(Some(&root_string), "bash").contains("generated/bash.sh"));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn shell_loader_install_and_uninstall_are_idempotent() {
    let root = std::env::temp_dir().join(format!("aliasmgr-loader-cli-{}", std::process::id()));
    let rc = root.join("bashrc");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(&rc, "user\n").unwrap();
    let mut config = aliasmgr_core::config::AppConfig::default();
    config.shells.bash_rc_path = Some(rc.clone());
    config.save(&aliasmgr_core::config::AppPaths { root: root.clone() }).unwrap();
    assert!(commands::shell_install(Some(root.to_str().unwrap()), "bash").unwrap());
    assert!(!commands::shell_install(Some(root.to_str().unwrap()), "bash").unwrap());
    assert!(commands::shell_install_status(Some(root.to_str().unwrap()), "bash").unwrap());
    assert!(commands::shell_uninstall(Some(root.to_str().unwrap()), "bash").unwrap());
    assert!(!commands::shell_uninstall(Some(root.to_str().unwrap()), "bash").unwrap());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn dry_run_does_not_create_database() {
    let root = std::env::temp_dir().join(format!("aliasmgr-dry-run-{}", std::process::id()));
    let root_string = root.to_string_lossy().to_string();
    let output = commands::sync(Some(&root_string), true).unwrap();
    assert!(output.is_empty());
    assert!(!root.join("aliases.db").exists());
    let _ = std::fs::remove_dir_all(root);
}
