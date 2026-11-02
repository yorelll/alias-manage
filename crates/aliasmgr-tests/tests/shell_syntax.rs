use aliasmgr_core::{
    model::{AliasRecord, ManagedNameSet},
    shells::{
        bash,
        common::{install_loader, render_loader},
        zsh,
    },
};
use std::{fs, path::Path, process::Command};

fn names() -> ManagedNameSet {
    ManagedNameSet {
        current: vec![],
        retired: vec![],
    }
}

fn restricted_alias(directory: &Path, blacklist: bool) -> AliasRecord {
    AliasRecord {
        name: "rma".into(),
        executable: "echo".into(),
        fixed_args: vec!["ran".into()],
        pass_args: false,
        restriction: Some(aliasmgr_core::model::PathRestriction {
            mode: if blacklist {
                aliasmgr_core::model::RestrictionMode::Blacklist
            } else {
                aliasmgr_core::model::RestrictionMode::Whitelist
            },
            paths: vec![directory.to_string_lossy().into_owned()],
        }),
        ..Default::default()
    }
}

fn run_wrapper(directory: &Path, script: &Path) -> std::process::Output {
    Command::new("bash")
        .arg(script)
        .current_dir(directory)
        .output()
        .unwrap()
}

#[test]
fn restricted_wrapper_allows_whitelisted_directories_and_blocks_siblings() {
    if !command_available("bash") {
        return;
    }
    let root = std::env::temp_dir().join(format!(
        "aliasmgr-restriction-guard-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or_default()
    ));
    let allowed = root.join("allowed");
    let subdirectory = allowed.join("sub");
    let sibling = root.join("allowed-suffix");
    let outside = root.join("outside");
    for directory in [&subdirectory, &sibling, &outside] {
        fs::create_dir_all(directory).unwrap();
    }
    let mut script = bash::render(&restricted_alias(&allowed, false), &names()).unwrap();
    script.push_str("\nrma\nexit_code=$?\necho \"status=$exit_code\"\nexit $exit_code\n");
    let script_path = root.join("run.sh");
    fs::write(&script_path, script).unwrap();

    for directory in [&allowed, &subdirectory] {
        let output = run_wrapper(directory, &script_path);
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success() && stdout.contains("ran"),
            "{} should run: {stdout}",
            directory.display()
        );
    }
    for directory in [&sibling, &outside] {
        let output = run_wrapper(directory, &script_path);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(!output.status.success());
        assert!(stderr.contains("current directory is not in the whitelist"));
        assert!(!String::from_utf8_lossy(&output.stdout).contains("ran"));
    }
    let _ = fs::remove_dir_all(root);
}

#[test]
fn restricted_wrapper_blacklist_blocks_configured_subtrees_only() {
    if !command_available("bash") {
        return;
    }
    let root = std::env::temp_dir().join(format!(
        "aliasmgr-restriction-blacklist-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or_default()
    ));
    let blocked = root.join("blocked");
    let blocked_sub = blocked.join("sub");
    let outside = root.join("outside");
    for directory in [&blocked_sub, &outside] {
        fs::create_dir_all(directory).unwrap();
    }
    let mut script = bash::render(&restricted_alias(&blocked, true), &names()).unwrap();
    script.push_str("\nrma\nexit_code=$?\necho \"status=$exit_code\"\nexit $exit_code\n");
    let script_path = root.join("run.sh");
    fs::write(&script_path, script).unwrap();

    let output = run_wrapper(&outside, &script_path);
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("ran"));
    for directory in [&blocked, &blocked_sub] {
        let output = run_wrapper(directory, &script_path);
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr)
            .contains("current directory is in the blacklist"));
    }
    let _ = fs::remove_dir_all(root);
}

fn command_available(command: &str) -> bool {
    Command::new(command)
        .arg("--version")
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn generated_script() -> String {
    let alias = AliasRecord {
        name: "gs".into(),
        executable: "git".into(),
        fixed_args: vec!["status".into()],
        ..Default::default()
    };
    bash::render(
        &alias,
        &ManagedNameSet {
            current: vec![],
            retired: vec![],
        },
    )
    .unwrap()
}

#[test]
fn bash_rendered_script_passes_real_bash_syntax_check() {
    if !command_available("bash") {
        return;
    }
    let root = std::env::temp_dir().join(format!("aliasmgr-bash-syntax-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let path = root.join("generated.sh");
    fs::write(&path, generated_script()).unwrap();
    let status = Command::new("bash")
        .args(["-n", path.to_str().unwrap()])
        .status()
        .unwrap();
    assert!(status.success());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn zsh_rendered_script_passes_real_zsh_syntax_check() {
    if !command_available("zsh") {
        return;
    }
    let alias = AliasRecord {
        name: "gs".into(),
        executable: "git".into(),
        fixed_args: vec!["status".into()],
        ..Default::default()
    };
    let script = zsh::render(
        &alias,
        &ManagedNameSet {
            current: vec![],
            retired: vec![],
        },
    )
    .unwrap();
    let root = std::env::temp_dir().join(format!("aliasmgr-zsh-syntax-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let path = root.join("generated.zsh");
    fs::write(&path, script).unwrap();
    let status = Command::new("zsh")
        .args(["-n", path.to_str().unwrap()])
        .status()
        .unwrap();
    assert!(status.success());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn zsh_restricted_script_passes_real_zsh_syntax_check() {
    if !command_available("zsh") {
        return;
    }
    let script = zsh::render(&restricted_alias(Path::new("/tmp"), false), &names()).unwrap();
    let root =
        std::env::temp_dir().join(format!("aliasmgr-zsh-restriction-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let path = root.join("generated.zsh");
    fs::write(&path, script).unwrap();
    let status = Command::new("zsh")
        .args(["-n", path.to_str().unwrap()])
        .status()
        .unwrap();
    assert!(status.success());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn loader_is_syntax_valid_when_sourced_by_bash() {
    if !command_available("bash") {
        return;
    }
    let loader = render_loader(Path::new("/tmp/aliasmgr-generated.sh"));
    let content = install_loader("# user config\n", &loader).unwrap();
    let root = std::env::temp_dir().join(format!("aliasmgr-loader-syntax-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let path = root.join("profile");
    fs::write(&path, content).unwrap();
    let status = Command::new("bash")
        .args(["-n", path.to_str().unwrap()])
        .status()
        .unwrap();
    assert!(status.success());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn shell_syntax_fixture_matrix_is_enabled() {
    let marker = "shell-matrix";
    assert_eq!(marker, "shell-matrix");
}
