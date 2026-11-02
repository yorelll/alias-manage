use aliasmgr_cli::cli::AddArgs;
use aliasmgr_cli::commands;
use aliasmgr_core::config::{AppConfig, AppPaths};
use aliasmgr_core::shells::common::{END_MARKER, START_MARKER};
use aliasmgr_core::uninstall::{uninstall, UninstallMode};

#[test]
fn uninstall_modes_preserve_referenced_targets() {
    let root = std::env::temp_dir().join(format!("aliasmgr-cli-uninstall-{}", std::process::id()));
    std::fs::create_dir_all(root.join("generated")).unwrap();
    let target = root.join("script.py");
    std::fs::write(&target, "print(1)\n").unwrap();
    std::fs::write(root.join("aliases.db"), "db").unwrap();
    uninstall(&root, UninstallMode::PurgeAliases, None).unwrap();
    assert!(target.exists());
    assert!(!root.join("aliases.db").exists());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn cli_uninstall_removes_marked_loader_blocks_and_keeps_user_content_and_targets() {
    let root = std::env::temp_dir().join(format!(
        "aliasmgr-cli-uninstall-loader-{}",
        std::process::id()
    ));
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
    assert!(
        !retained.contains(START_MARKER),
        "loader block survived: {retained}"
    );
    assert!(
        !retained.contains(END_MARKER),
        "loader block survived: {retained}"
    );
    assert!(retained.contains("user before"));
    assert!(retained.contains("user after"));
    assert!(rc.exists());
    assert!(target.exists());

    // A real alias pointing at script.py: purge must drop managed files but keep the target.
    let add = AddArgs {
        name: "runpy".into(),
        exec: target.to_string_lossy().into_owned(),
        args: vec![],
        shell: vec!["bash".into()],
        cwd: None,
        environment: vec![],
        tags: vec![],
        description: None,
        restrict: None,
        restrict_paths: vec![],
        no_pass_args: false,
        target_type: None,
        path_mode: None,
    };
    commands::add(Some(&config_dir), &add).unwrap();
    commands::sync(Some(&config_dir), false).unwrap();
    assert!(root.join("aliases.db").exists());
    assert!(root.join("generated/bash.sh").exists());

    commands::uninstall(Some(&config_dir), true).unwrap();
    assert!(
        target.exists(),
        "purge must never delete the referenced target"
    );
    assert!(rc.exists());
    assert!(!root.join("aliases.db").exists());
    assert!(!root.join("generated/bash.sh").exists());
    assert!(!std::fs::read_to_string(&rc).unwrap().contains(START_MARKER));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn uninstall_purges_even_when_config_is_unparsable() {
    let root = std::env::temp_dir().join(format!(
        "aliasmgr-cli-uninstall-badcfg-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(root.join("generated")).unwrap();
    std::fs::write(root.join("aliases.db"), "db").unwrap();
    std::fs::write(root.join("generated/bash.sh"), "generated\n").unwrap();
    let rc = root.join(".bashrc");
    std::fs::write(
        &rc,
        format!("before\n{START_MARKER}\nblock\n{END_MARKER}\nafter\n"),
    )
    .unwrap();
    std::fs::write(root.join("config.toml"), "this is not toml = = =\n").unwrap();

    commands::uninstall(Some(&root.to_string_lossy()), true).unwrap();
    assert!(!root.join("aliases.db").exists());
    assert!(!root.join("generated/bash.sh").exists());
    let content = std::fs::read_to_string(&rc).unwrap();
    assert_eq!(content, "before\nafter\n");
    let _ = std::fs::remove_dir_all(root);
}
