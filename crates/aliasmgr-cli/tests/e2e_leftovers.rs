use aliasmgr_core::config::{AppConfig, AppPaths};
use std::{
    path::{Path, PathBuf},
    process::{Command, Output},
};

fn temp_root(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "aliasmgr-e2e-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or_default()
    ))
}

fn run(config: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_aliasmgr"))
        .arg("--config-dir")
        .arg(config)
        .args(args)
        .output()
        .unwrap()
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn binary_accepts_target_type_and_path_mode_and_rejects_raw_shell_command() {
    let root = temp_root("target-options");
    std::fs::create_dir_all(&root).unwrap();

    let add = run(
        &root,
        &[
            "add",
            "gs",
            "--exec",
            "git",
            "--target-type",
            "java_jar",
            "--path-mode",
            "relative_to_working_directory",
        ],
    );
    assert!(add.status.success(), "add failed: {}", stderr(&add));

    let get = run(&root, &["get", "gs"]);
    assert!(get.status.success(), "get failed: {}", stderr(&get));
    let record = stdout(&get);
    assert!(record.contains("target_type: JavaJar"), "{record}");
    assert!(
        record.contains("path_mode: RelativeToWorkingDirectory"),
        "{record}"
    );

    let update = run(
        &root,
        &[
            "update",
            "gs",
            "--target-type",
            "batch",
            "--path-mode",
            "absolute",
        ],
    );
    assert!(
        update.status.success(),
        "update failed: {}",
        stderr(&update)
    );
    let record = stdout(&run(&root, &["get", "gs"]));
    assert!(record.contains("target_type: Batch"), "{record}");
    assert!(record.contains("path_mode: Absolute"), "{record}");

    let rejected = run(
        &root,
        &[
            "add",
            "raw",
            "--exec",
            "git",
            "--target-type",
            "raw_shell_command",
        ],
    );
    assert_eq!(
        rejected.status.code(),
        Some(2),
        "clap must reject the value"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn binary_installs_and_removes_the_powershell_loader_in_a_temp_profile() {
    let root = temp_root("ps-loader");
    let profile = root.join("profile.ps1");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(&profile, "user before\n").unwrap();
    let mut config = AppConfig::default();
    config.shells.powershell7_profile_path = Some(profile.clone());
    config.save(&AppPaths { root: root.clone() }).unwrap();

    let install = run(&root, &["shell", "install", "powershell7"]);
    assert!(
        install.status.success(),
        "install failed: {}",
        stderr(&install)
    );
    let content = std::fs::read_to_string(&profile).unwrap();
    assert!(content.contains("user before"));
    assert!(content.contains("# >>> Alias Manager >>>"));
    assert!(content.contains("# <<< Alias Manager <<<"));
    assert!(content.contains("Test-Path -LiteralPath $__aliasmgr_generated_file"));
    assert!(content.contains("powershell7.ps1"));
    assert_eq!(content.matches("# >>> Alias Manager >>>").count(), 1);

    let again = run(&root, &["shell", "install", "powershell7"]);
    assert!(again.status.success());
    assert!(stdout(&again).contains("已存在"));
    assert_eq!(
        std::fs::read_to_string(&profile)
            .unwrap()
            .matches("# >>> Alias Manager >>>")
            .count(),
        1
    );

    let uninstall = run(&root, &["shell", "uninstall", "powershell7"]);
    assert!(
        uninstall.status.success(),
        "uninstall failed: {}",
        stderr(&uninstall)
    );
    assert_eq!(std::fs::read_to_string(&profile).unwrap(), "user before\n");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn binary_writes_crlf_powershell5_generated_file_when_configured() {
    let root = temp_root("crlf");
    std::fs::create_dir_all(&root).unwrap();
    let mut config = AppConfig::default();
    config.runtime.powershell5_line_ending = Some("crlf".into());
    config.save(&AppPaths { root: root.clone() }).unwrap();

    let add = run(
        &root,
        &["add", "gs", "--exec", "git", "--shell", "powershell5"],
    );
    assert!(add.status.success(), "add failed: {}", stderr(&add));
    let sync = run(&root, &["sync"]);
    assert!(sync.status.success(), "sync failed: {}", stderr(&sync));

    let bytes = std::fs::read(root.join("generated/powershell5.ps1")).unwrap();
    assert!(bytes.starts_with(&[0xEF, 0xBB, 0xBF]), "expected UTF-8 BOM");
    assert!(
        bytes.windows(2).any(|window| window == b"\r\n"),
        "expected CRLF bytes"
    );
    assert!(
        !bytes
            .windows(2)
            .any(|window| window[1] == b'\n' && window[0] != b'\r'),
        "expected no bare LF bytes"
    );
    let content = String::from_utf8(bytes).unwrap();
    let checksum = content
        .lines()
        .find_map(|line| line.strip_prefix("# file_checksum: "))
        .unwrap();
    assert!(aliasmgr_core::sync::verify_file_checksum(
        &content, checksum
    ));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn binary_doctor_reports_a_tampered_generated_file() {
    let root = temp_root("doctor-tamper");
    std::fs::create_dir_all(&root).unwrap();
    let add = run(&root, &["add", "gs", "--exec", "git", "--shell", "bash"]);
    assert!(add.status.success(), "add failed: {}", stderr(&add));
    let sync = run(&root, &["sync"]);
    assert!(sync.status.success(), "sync failed: {}", stderr(&sync));

    let generated = root.join("generated/bash.sh");
    let mut tampered = std::fs::read_to_string(&generated).unwrap();
    tampered.push_str("# manual edit\n");
    std::fs::write(&generated, tampered).unwrap();

    let doctor = run(&root, &["doctor"]);
    assert!(
        doctor.status.success(),
        "doctor failed: {}",
        stderr(&doctor)
    );
    assert!(
        stdout(&doctor).contains("生成文件校验和不匹配（可能被手工修改）"),
        "stdout: {}",
        stdout(&doctor)
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn binary_rejects_relative_restriction_path_with_validation_exit_code() {
    let root = temp_root("restriction-exit");
    std::fs::create_dir_all(&root).unwrap();
    let output = run(
        &root,
        &[
            "add",
            "rma",
            "--exec",
            "rm",
            "--restrict",
            "whitelist",
            "--restrict-path",
            "relative/path",
        ],
    );
    assert_eq!(output.status.code(), Some(5), "stderr: {}", stderr(&output));
    assert!(
        stderr(&output).contains("absolute path for this platform"),
        "stderr: {}",
        stderr(&output)
    );
    assert!(!root.join("aliases.db").exists());
    let _ = std::fs::remove_dir_all(root);
}
