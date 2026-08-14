use aliasmgr_cli::commands;

#[test]
fn doctor_reports_missing_configuration_and_reload_print_is_stable() {
    let root = std::env::temp_dir().join(format!("aliasmgr-doctor-{}", std::process::id()));
    let root_string = root.to_string_lossy().to_string();
    let findings = commands::doctor(Some(&root_string)).unwrap();
    assert!(findings.iter().any(|finding| finding.contains("数据库")));
    assert_eq!(commands::reload_print(Some(&root_string), "bash"), format!("source '{root_string}/generated/bash.sh'"));
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
