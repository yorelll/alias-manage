use aliasmgr_cli::commands;

#[test]
fn doctor_reports_missing_configuration_and_reload_print_is_stable() {
    let root = std::env::temp_dir().join(format!("aliasmgr-doctor-{}", std::process::id()));
    let root_string = root.to_string_lossy().to_string();
    let findings = commands::doctor(Some(&root_string)).unwrap();
    assert!(findings.iter().any(|finding| finding.contains("数据库")));
    assert!(commands::reload_print(Some(&root_string), "bash").contains("generated/bash.sh"));
    assert!(commands::reload_print(Some(&root_string), "zsh").contains("generated/zsh.sh"));
    assert!(commands::reload_print(Some(&root_string), "powershell5")
        .contains("generated/powershell5.ps1"));
    assert!(commands::reload_print(Some(&root_string), "powershell7")
        .contains("generated/powershell7.ps1"));
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
    config
        .save(&aliasmgr_core::config::AppPaths { root: root.clone() })
        .unwrap();
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

fn temp_root(label: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "aliasmgr-loader-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or_default()
    ))
}

#[test]
fn powershell5_loader_install_and_uninstall_use_the_configured_profile() {
    let root = temp_root("ps5");
    let profile = root.join("profile.ps1");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(&profile, "user before\n").unwrap();
    let mut config = aliasmgr_core::config::AppConfig::default();
    config.shells.powershell5_profile_path = Some(profile.clone());
    config
        .save(&aliasmgr_core::config::AppPaths { root: root.clone() })
        .unwrap();

    let config_dir = root.to_string_lossy().to_string();
    assert!(commands::shell_install(Some(&config_dir), "powershell5").unwrap());
    assert!(!commands::shell_install(Some(&config_dir), "powershell5").unwrap());
    assert!(commands::shell_install_status(Some(&config_dir), "powershell5").unwrap());

    let content = std::fs::read_to_string(&profile).unwrap();
    assert!(content.contains("user before"));
    assert!(content.contains(aliasmgr_core::shells::common::START_MARKER));
    assert!(content.contains(aliasmgr_core::shells::common::END_MARKER));
    assert!(content.contains("Test-Path -LiteralPath $__aliasmgr_generated_file"));
    assert!(content.contains(". $__aliasmgr_generated_file"));
    assert!(content.contains("powershell5.ps1"));
    assert_eq!(
        content
            .matches(aliasmgr_core::shells::common::START_MARKER)
            .count(),
        1
    );

    assert!(commands::shell_uninstall(Some(&config_dir), "powershell5").unwrap());
    assert!(!commands::shell_uninstall(Some(&config_dir), "powershell5").unwrap());
    assert_eq!(std::fs::read_to_string(&profile).unwrap(), "user before\n");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn powershell7_loader_install_and_uninstall_use_the_configured_profile() {
    let root = temp_root("ps7");
    let profile = root.join("profile.ps1");
    std::fs::create_dir_all(&root).unwrap();
    let mut config = aliasmgr_core::config::AppConfig::default();
    config.shells.powershell7_profile_path = Some(profile.clone());
    config
        .save(&aliasmgr_core::config::AppPaths { root: root.clone() })
        .unwrap();

    let config_dir = root.to_string_lossy().to_string();
    assert!(commands::shell_install(Some(&config_dir), "powershell7").unwrap());
    assert!(!commands::shell_install(Some(&config_dir), "powershell7").unwrap());
    let content = std::fs::read_to_string(&profile).unwrap();
    assert!(content.contains("powershell7.ps1"));
    assert!(!content.contains("[ -r"));

    assert!(commands::shell_uninstall(Some(&config_dir), "powershell7").unwrap());
    assert_eq!(std::fs::read_to_string(&profile).unwrap(), "");
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(not(windows))]
#[test]
fn powershell5_without_a_configured_profile_is_not_installed_on_non_windows() {
    let root = temp_root("ps5-unix");
    std::fs::create_dir_all(&root).unwrap();
    let config_dir = root.to_string_lossy().to_string();
    assert!(matches!(
        commands::shell_install(Some(&config_dir), "powershell5"),
        Err(aliasmgr_core::error::AliasError::ShellNotInstalled)
    ));
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn shell_install_through_a_symlinked_rc_updates_the_target_and_preserves_the_link() {
    use std::os::unix::fs::symlink;

    let root = temp_root("symlink");
    std::fs::create_dir_all(&root).unwrap();
    let target = root.join("real-bashrc");
    let link = root.join("bashrc");
    std::fs::write(&target, "user before\n").unwrap();
    symlink(&target, &link).unwrap();
    let mut config = aliasmgr_core::config::AppConfig::default();
    config.shells.bash_rc_path = Some(link.clone());
    config
        .save(&aliasmgr_core::config::AppPaths { root: root.clone() })
        .unwrap();

    let config_dir = root.to_string_lossy().to_string();
    assert!(commands::shell_install(Some(&config_dir), "bash").unwrap());
    assert!(
        link.symlink_metadata().unwrap().file_type().is_symlink(),
        "install must preserve the symlink"
    );
    assert!(std::fs::read_to_string(&target)
        .unwrap()
        .contains(aliasmgr_core::shells::common::START_MARKER));
    assert_eq!(
        std::fs::read_to_string(&link).unwrap(),
        std::fs::read_to_string(&target).unwrap()
    );

    assert!(commands::shell_uninstall(Some(&config_dir), "bash").unwrap());
    assert!(
        link.symlink_metadata().unwrap().file_type().is_symlink(),
        "uninstall must preserve the symlink"
    );
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "user before\n");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn doctor_reports_manual_generated_file_edits() {
    use aliasmgr_core::model::{AliasRecord, ShellKind};

    let root = temp_root("doctor-tamper");
    let config_dir = root.to_string_lossy().to_string();
    let alias = AliasRecord {
        name: "gs".into(),
        executable: "git".into(),
        ..Default::default()
    };
    aliasmgr_core::sync::SyncCoordinator::new(&root)
        .apply(&[alias], &[ShellKind::Bash], 2)
        .unwrap();
    let generated = root.join("generated/bash.sh");

    let clean = commands::doctor(Some(&config_dir)).unwrap();
    assert!(
        !clean.iter().any(|finding| finding.contains("校验和不匹配")),
        "clean generated file must not be reported: {clean:?}"
    );

    let mut tampered = std::fs::read_to_string(&generated).unwrap();
    tampered.push_str("# manual edit\n");
    std::fs::write(&generated, tampered).unwrap();

    let findings = commands::doctor(Some(&config_dir)).unwrap();
    assert!(
        findings
            .iter()
            .any(|finding| finding.contains("生成文件校验和不匹配（可能被手工修改）")),
        "tampered file must be reported: {findings:?}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn powershell_loader_install_creates_a_missing_profile_parent() {
    let root = temp_root("ps7-missing-parent");
    let profile = root.join("missing").join("nested").join("profile.ps1");
    std::fs::create_dir_all(&root).unwrap();
    let mut config = aliasmgr_core::config::AppConfig::default();
    config.shells.powershell7_profile_path = Some(profile.clone());
    config
        .save(&aliasmgr_core::config::AppPaths { root: root.clone() })
        .unwrap();
    assert!(!profile.parent().unwrap().exists());

    let config_dir = root.to_string_lossy().to_string();
    assert!(commands::shell_install(Some(&config_dir), "powershell7").unwrap());
    assert!(profile.exists());
    assert!(std::fs::read_to_string(&profile)
        .unwrap()
        .contains(aliasmgr_core::shells::common::START_MARKER));
    let _ = std::fs::remove_dir_all(root);
}
