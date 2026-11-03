use aliasmgr_cli::cli::{AddArgs, UpdateArgs};
use aliasmgr_cli::commands;
use aliasmgr_cli::exit_code;
use aliasmgr_core::config::{AppConfig, AppPaths};
use aliasmgr_core::error::AliasError;
use aliasmgr_core::model::ShellKind;
use aliasmgr_core::shell_setup;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn temp_root(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "aliasmgr-shell-integration-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or_default()
    ));
    std::fs::create_dir_all(&root).unwrap();
    root
}

fn add_args(name: &str, shell: &str) -> AddArgs {
    AddArgs {
        name: name.into(),
        exec: "git".into(),
        args: vec!["status".into()],
        shell: vec![shell.into()],
        cwd: None,
        environment: vec![],
        tags: vec![],
        description: None,
        restrict: None,
        restrict_paths: vec![],
        no_pass_args: false,
        target_type: None,
        path_mode: None,
        no_sync: false,
        allow_missing_loader: false,
    }
}

fn update_args(name: &str) -> UpdateArgs {
    UpdateArgs {
        name: name.into(),
        exec: None,
        args: None,
        shell: None,
        cwd: None,
        environment: None,
        tags: None,
        description: None,
        restrict: None,
        restrict_paths: vec![],
        clear_restriction: false,
        no_pass_args: false,
        pass_args: false,
        target_type: None,
        path_mode: None,
        no_sync: false,
        allow_missing_loader: false,
    }
}

/// Enables Bash and installs its loader into an isolated profile.
fn configure_bash(root: &Path) -> AppPaths {
    let paths = AppPaths {
        root: root.to_path_buf(),
    };
    let mut config = AppConfig::default();
    config.shells.enabled = vec!["bash".into()];
    config.shells.bash_rc_path = Some(root.join("bashrc"));
    config.save(&paths).unwrap();
    aliasmgr_core::integration::install_loader(&paths, "bash").unwrap();
    paths
}

fn generated(root: &Path) -> PathBuf {
    root.join("generated/bash.sh")
}

fn fake_executable(root: &Path) -> PathBuf {
    let path = root.join(if cfg!(windows) {
        "fake-bash.exe"
    } else {
        "fake-bash"
    });
    std::fs::write(&path, b"#!/bin/sh\nexit 0\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    path
}

fn missing_manual_config(
    root: &Path,
    keep_bash: bool,
    bash_executable: Option<&Path>,
) -> AppConfig {
    let mut config = AppConfig::default();
    for kind in [
        ShellKind::Bash,
        ShellKind::Zsh,
        ShellKind::PowerShell5,
        ShellKind::PowerShell7,
    ] {
        if keep_bash && kind == ShellKind::Bash {
            if let Some(executable) = bash_executable {
                config
                    .shells
                    .executables
                    .insert("bash".into(), executable.to_path_buf());
            }
            continue;
        }
        config
            .shells
            .executables
            .insert(kind.as_str().into(), root.join("missing"));
    }
    config.shells.bash_rc_path = Some(root.join("bashrc"));
    config
}

#[test]
fn auto_sync_writes_affected_shell_after_add() {
    let root = temp_root("auto-add");
    configure_bash(&root);
    let config_dir = root.to_string_lossy().to_string();
    commands::add(Some(&config_dir), &add_args("gs", "bash")).unwrap();
    let content = std::fs::read_to_string(generated(&root)).unwrap();
    assert!(
        content.contains("gs() { command 'git' 'status'"),
        "{content}"
    );
    assert!(!content.contains("revision: 0"), "{content}");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn no_sync_and_auto_sync_false_both_skip_the_post_save_sync() {
    let root = temp_root("no-sync");
    configure_bash(&root);
    let config_dir = root.to_string_lossy().to_string();

    let mut args = add_args("gs", "bash");
    args.no_sync = true;
    commands::add(Some(&config_dir), &args).unwrap();
    assert!(
        !generated(&root).exists(),
        "--no-sync must not generate files"
    );

    let paths = AppPaths { root: root.clone() };
    let mut config = paths.load_config().unwrap();
    config.runtime.auto_sync = false;
    config.save(&paths).unwrap();
    commands::add(Some(&config_dir), &add_args("gd", "bash")).unwrap();
    assert!(
        !generated(&root).exists(),
        "runtime.auto_sync = false must not generate files"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn add_preflight_refuses_unenabled_or_missing_loader_targets() {
    let root = temp_root("preflight");
    let config_dir = root.to_string_lossy().to_string();

    let error = commands::add(Some(&config_dir), &add_args("gs", "bash")).unwrap_err();
    assert!(matches!(error, AliasError::ShellNotReady(_)));
    assert_eq!(exit_code::exit_code(&error), 12);
    assert!(commands::get(Some(&config_dir), "gs").is_err());

    // Enabled but loader missing is also refused.
    let paths = AppPaths { root: root.clone() };
    let mut config = AppConfig::default();
    config.shells.enabled = vec!["bash".into()];
    config.shells.bash_rc_path = Some(root.join("bashrc"));
    config.save(&paths).unwrap();
    let error = commands::add(Some(&config_dir), &add_args("gs", "bash")).unwrap_err();
    assert!(
        matches!(&error, AliasError::ShellNotReady(message) if message.contains("loader")),
        "{error}"
    );
    assert!(commands::get(Some(&config_dir), "gs").is_err());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn allow_missing_loader_saves_without_affecting_non_target_shells() {
    let root = temp_root("allow");
    let config_dir = root.to_string_lossy().to_string();
    let mut args = add_args("gs", "bash");
    args.allow_missing_loader = true;
    commands::add(Some(&config_dir), &args).unwrap();
    assert_eq!(
        commands::get(Some(&config_dir), "gs").unwrap().shells,
        vec![ShellKind::Bash]
    );

    // With Bash integrated, a Zsh target that is not enabled is still checked:
    // preflight must look only at the shells this record targets.
    let root2 = temp_root("allow-targets");
    configure_bash(&root2);
    let config_dir2 = root2.to_string_lossy().to_string();
    let mut multi = add_args("gm", "bash");
    multi.shell.push("zsh".into());
    assert!(matches!(
        commands::add(Some(&config_dir2), &multi),
        Err(AliasError::ShellNotReady(message)) if message.contains("Zsh")
    ));
    let mut bash_only = add_args("gb", "bash");
    bash_only.shell = vec!["bash".into()];
    commands::add(Some(&config_dir2), &bash_only).unwrap();
    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(root2);
}

#[test]
fn no_sync_still_checks_enabled_but_skips_the_loader_probe() {
    let root = temp_root("no-sync-preflight");
    let paths = AppPaths { root: root.clone() };
    let mut config = AppConfig::default();
    config.shells.enabled = vec!["bash".into()];
    config.shells.bash_rc_path = Some(root.join("bashrc"));
    config.save(&paths).unwrap();
    let config_dir = root.to_string_lossy().to_string();

    let mut args = add_args("gs", "bash");
    args.no_sync = true;
    commands::add(Some(&config_dir), &args).unwrap();
    assert!(!generated(&root).exists());

    config.shells.enabled.clear();
    config.save(&paths).unwrap();
    let mut args = add_args("gd", "bash");
    args.no_sync = true;
    assert!(matches!(
        commands::add(Some(&config_dir), &args),
        Err(AliasError::ShellNotReady(_))
    ));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn partial_sync_failure_keeps_the_database_write_and_returns_exit_10() {
    let root = temp_root("partial");
    let config_dir = root.to_string_lossy().to_string();
    std::fs::create_dir_all(&root).unwrap();
    // A file where the generated directory should be makes the per-shell sync fail.
    std::fs::write(root.join("generated"), b"not a directory").unwrap();
    let mut args = add_args("gs", "bash");
    args.allow_missing_loader = true;
    let error = commands::add(Some(&config_dir), &args).unwrap_err();
    assert!(
        matches!(&error, AliasError::PartialSync(message) if message.contains("saved")),
        "{error}"
    );
    assert_eq!(exit_code::exit_code(&error), 10);
    assert_eq!(commands::get(Some(&config_dir), "gs").unwrap().name, "gs");

    // --no-sync does not touch the broken generated path and exits 0.
    let mut args = add_args("gd", "bash");
    args.allow_missing_loader = true;
    args.no_sync = true;
    commands::add(Some(&config_dir), &args).unwrap();
    assert_eq!(commands::get(Some(&config_dir), "gd").unwrap().name, "gd");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn enable_disable_rename_and_remove_auto_sync_their_bound_shells() {
    let root = temp_root("lifecycle-sync");
    configure_bash(&root);
    let config_dir = root.to_string_lossy().to_string();
    commands::add(Some(&config_dir), &add_args("gs", "bash")).unwrap();
    let before = std::fs::read_to_string(generated(&root)).unwrap();
    assert!(before.contains("gs() { command 'git' 'status'"), "{before}");

    commands::enable(Some(&config_dir), "gs", false).unwrap();
    let after_disable = std::fs::read_to_string(generated(&root)).unwrap();
    assert_ne!(before, after_disable);
    assert!(
        !after_disable.contains("gs() { command 'git' 'status'"),
        "{after_disable}"
    );

    commands::enable(Some(&config_dir), "gs", true).unwrap();
    assert!(std::fs::read_to_string(generated(&root))
        .unwrap()
        .contains("gs() { command 'git' 'status'"));

    commands::rename(Some(&config_dir), "gs", "gd").unwrap();
    let after_rename = std::fs::read_to_string(generated(&root)).unwrap();
    assert!(
        after_rename.contains("gd() { command 'git' 'status'"),
        "{after_rename}"
    );
    assert!(
        !after_rename.contains("gs() { command 'git' 'status'"),
        "{after_rename}"
    );

    commands::remove(Some(&config_dir), "gd").unwrap();
    let after_remove = std::fs::read_to_string(generated(&root)).unwrap();
    assert!(
        !after_remove.contains("gd() { command 'git' 'status'"),
        "{after_remove}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn update_preflight_rejects_a_new_unenabled_shell_before_writing() {
    let root = temp_root("update-preflight");
    configure_bash(&root);
    let config_dir = root.to_string_lossy().to_string();
    commands::add(Some(&config_dir), &add_args("gs", "bash")).unwrap();

    let mut change = update_args("gs");
    change.shell = Some(vec!["zsh".into()]);
    let error = commands::update(Some(&config_dir), &change).unwrap_err();
    assert!(
        matches!(&error, AliasError::ShellNotReady(message) if message.contains("Zsh")),
        "{error}"
    );
    assert_eq!(
        commands::get(Some(&config_dir), "gs").unwrap().shells,
        vec![ShellKind::Bash]
    );

    // The escape hatch saves and warns; the sync then targets the new shell too.
    let mut change = update_args("gs");
    change.shell = Some(vec!["zsh".into()]);
    change.allow_missing_loader = true;
    change.no_sync = true;
    commands::update(Some(&config_dir), &change).unwrap();
    assert_eq!(
        commands::get(Some(&config_dir), "gs").unwrap().shells,
        vec![ShellKind::Zsh]
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn doctor_interactive_detection_is_strictly_both_streams() {
    assert!(!commands::doctor_is_interactive(false, false));
    assert!(!commands::doctor_is_interactive(true, false));
    assert!(!commands::doctor_is_interactive(false, true));
    assert!(commands::doctor_is_interactive(true, true));
}

#[test]
fn doctor_json_is_machine_readable_and_keeps_finding_codes() {
    let root = temp_root("doctor-json");
    let config_dir = root.to_string_lossy().to_string();
    let report = commands::doctor_json(Some(&config_dir)).unwrap();
    let value: serde_json::Value = serde_json::from_str(&report).unwrap();
    let findings = value.as_array().expect("JSON array");
    assert!(findings
        .iter()
        .any(|finding| finding["code"] == "database_missing"));
    assert!(findings
        .iter()
        .all(|finding| finding.get("severity").is_some() && finding.get("message").is_some()));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn binary_doctor_format_json_never_enters_the_interactive_screen() {
    let root = temp_root("doctor-json-bin");
    let output: Output = Command::new(env!("CARGO_BIN_EXE_aliasmgr"))
        .arg("--config-dir")
        .arg(&root)
        .args(["doctor", "--format", "json"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "doctor failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout must be JSON");
    assert!(value.as_array().is_some());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn binary_add_preflight_exits_12_and_allow_missing_loader_warns() {
    let root = temp_root("preflight-bin");
    let base = [
        "--config-dir",
        root.to_str().unwrap(),
        "add",
        "gs",
        "--exec",
        "git",
        "--shell",
        "bash",
    ];
    let refused = Command::new(env!("CARGO_BIN_EXE_aliasmgr"))
        .args(base)
        .output()
        .unwrap();
    assert_eq!(refused.status.code(), Some(12));
    let stderr = String::from_utf8_lossy(&refused.stderr);
    assert!(stderr.contains("aliasmgr doctor"), "{stderr}");

    let mut allowed = base.to_vec();
    allowed.push("--allow-missing-loader");
    let output = Command::new(env!("CARGO_BIN_EXE_aliasmgr"))
        .args(allowed)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "add failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("will not take effect"), "{stderr}");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn line_fallback_parses_selection_and_can_quit_without_writing() {
    let root = temp_root("line-quit");
    let paths = AppPaths { root: root.clone() };
    missing_manual_config(&root, false, None)
        .save(&paths)
        .unwrap();
    let mut input = Cursor::new("q\n");
    let mut output = Vec::new();
    let summary = commands::doctor_interactive_line(&paths, &mut input, &mut output).unwrap();
    assert_eq!(summary, "doctor: no changes");
    let printed = String::from_utf8(output).unwrap();
    assert!(printed.contains("Bash"), "{printed}");
    assert!(printed.contains("not detected"), "{printed}");
    assert!(paths.load_config().unwrap().shells.enabled.is_empty());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn line_fallback_cancels_a_not_detected_shell_without_writing() {
    let root = temp_root("line-cancel");
    let paths = AppPaths { root: root.clone() };
    missing_manual_config(&root, false, None)
        .save(&paths)
        .unwrap();
    let mut input = Cursor::new("bash\n3\n");
    let mut output = Vec::new();
    let summary = commands::doctor_interactive_line(&paths, &mut input, &mut output).unwrap();
    assert_eq!(summary, "doctor: no changes");
    assert!(!root.join("bashrc").exists());
    assert!(paths.load_config().unwrap().shells.enabled.is_empty());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn line_fallback_previews_and_applies_a_detected_shell() {
    let root = temp_root("line-apply");
    let paths = AppPaths { root: root.clone() };
    let executable = fake_executable(&root);
    missing_manual_config(&root, true, Some(&executable))
        .save(&paths)
        .unwrap();
    let mut input = Cursor::new("bash\ny\n");
    let mut output = Vec::new();
    let summary = commands::doctor_interactive_line(&paths, &mut input, &mut output).unwrap();
    let printed = String::from_utf8(output).unwrap();
    assert!(
        printed.contains("Preview") || printed.contains("Target:"),
        "{printed}"
    );
    assert!(summary.contains("Bash: enabled"), "{summary}");
    assert!(summary.contains("loader installed"), "{summary}");
    let config = paths.load_config().unwrap();
    assert_eq!(config.shells.enabled, vec!["bash"]);
    assert!(root.join("bashrc").exists());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn parse_shell_selection_is_strict_about_supported_spellings() {
    assert_eq!(
        commands::parse_shell_selection("bash, powershell7 bash").unwrap(),
        vec![ShellKind::Bash, ShellKind::PowerShell7]
    );
    assert!(commands::parse_shell_selection("fish").is_err());
    assert!(commands::parse_shell_selection("power_shell5").is_err());
    assert!(commands::parse_shell_selection("  ").unwrap().is_empty());
}

#[test]
fn shell_setup_enabled_intent_is_shared_with_the_config() {
    let root = temp_root("enabled-intent");
    let paths = AppPaths { root: root.clone() };
    shell_setup::set_enabled_shells(&paths, &[ShellKind::PowerShell7, ShellKind::Bash]).unwrap();
    assert_eq!(
        shell_setup::enabled_shells(&paths.load_config().unwrap()),
        vec![ShellKind::Bash, ShellKind::PowerShell7]
    );
    let _ = std::fs::remove_dir_all(root);
}
