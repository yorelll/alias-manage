use aliasmgr_cli::commands;
use aliasmgr_core::config::{AppConfig, AppPaths};
use aliasmgr_core::shells::common::{END_MARKER, START_MARKER};
use aliasmgr_core::uninstall::{uninstall, UninstallMode};

#[test]
fn uninstall_modes_preserve_referenced_targets() {
    let root = std::env::temp_dir().join(format!("aliasmgr-cli-uninstall-{}", std::process::id()));
    std::fs::create_dir_all(root.join("generated")).unwrap();
    let target = root.join("script.py"); std::fs::write(&target, "print(1)\n").unwrap();
    std::fs::write(root.join("aliases.db"), "db").unwrap();
    uninstall(&root, UninstallMode::PurgeAliases, None).unwrap();
    assert!(target.exists()); assert!(!root.join("aliases.db").exists());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn cli_uninstall_removes_marked_loader_blocks_and_keeps_user_content_and_targets() {
    let root = std::env::temp_dir().join(format!("aliasmgr-cli-uninstall-loader-{}", std::process::id()));
    std::fs::create_dir_all(root.join("generated")).unwrap();
    let rc = root.join("custom_rc");
    let marked = format!("user before\n{START_MARKER}\n# managed block, do not edit by hand\nexport X=1\n{END_MARKER}\nuser after\n");
    std::fs::write(&rc, marked).unwrap();
    let target = root.join("script.py");
    std::fs::write(&target, "print(1)\n").unwrap();
    let mut config = AppConfig::default();
    config.shells.bash_rc_path = Some(rc.clone());
    config.save(&AppPaths { root: root.clone() }).unwrap();

    let config_dir = root.to_string_lossy().to_string();
    commands::uninstall(Some(&config_dir), false).unwrap();
    let retained = std::fs::read_to_string(&rc).unwrap();
    assert!(!retained.contains(START_MARKER), "loader block survived: {retained}");
    assert!(!retained.contains(END_MARKER), "loader block survived: {retained}");
    assert!(retained.contains("user before"));
    assert!(retained.contains("user after"));
    assert!(rc.exists());
    assert!(target.exists());

    commands::uninstall(Some(&config_dir), true).unwrap();
    assert!(target.exists());
    assert!(rc.exists());
    assert!(!std::fs::read_to_string(&rc).unwrap().contains(START_MARKER));
    let _ = std::fs::remove_dir_all(root);
}
