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
