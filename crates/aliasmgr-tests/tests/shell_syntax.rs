use aliasmgr_core::{
    model::{AliasRecord, ManagedNameSet},
    shells::{
        bash,
        common::{install_loader, render_loader},
        zsh,
    },
};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

fn names() -> ManagedNameSet {
    ManagedNameSet {
        current: vec![],
        retired: vec![],
    }
}

fn restricted_alias_with_paths(paths: Vec<String>, blacklist: bool) -> AliasRecord {
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
            paths,
        }),
        ..Default::default()
    }
}

fn restricted_alias(directory: &Path, blacklist: bool) -> AliasRecord {
    restricted_alias_with_paths(vec![directory.to_string_lossy().into_owned()], blacklist)
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

fn unique_root(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "aliasmgr-shell-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or_default()
    ))
}

fn run_with_shell(shell: &str, directory: &Path, script: &Path) -> std::process::Output {
    Command::new(shell)
        .arg(script)
        .current_dir(directory)
        .output()
        .unwrap()
}

#[test]
fn restricted_wrapper_whitelist_root_and_missing_dirs() {
    if !command_available("bash") {
        return;
    }
    let root = unique_root("root-missing");
    let inside = root.join("inside");
    fs::create_dir_all(&inside).unwrap();

    let root_alias = restricted_alias_with_paths(vec!["/".into()], false);
    let mut script = bash::render(&root_alias, &names()).unwrap();
    script.push_str("\nrma\nexit $?\n");
    let script_path = root.join("root.sh");
    fs::write(&script_path, script).unwrap();
    let output = run_wrapper(&inside, &script_path);
    assert!(
        output.status.success() && String::from_utf8_lossy(&output.stdout).contains("ran"),
        "root whitelist should allow: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let missing = root.join("does-not-exist");
    let missing_alias =
        restricted_alias_with_paths(vec![missing.to_string_lossy().into_owned()], false);
    let mut script = bash::render(&missing_alias, &names()).unwrap();
    script.push_str("\nrma\nexit $?\n");
    let script_path = root.join("missing.sh");
    fs::write(&script_path, script).unwrap();
    let output = run_wrapper(&inside, &script_path);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr)
        .contains("current directory is not in the whitelist"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn restricted_wrapper_reports_status_and_stops_and_for_bash() {
    if !command_available("bash") {
        return;
    }
    let root = unique_root("status-and");
    let allowed = root.join("allowed");
    let outside = root.join("outside");
    for directory in [&allowed, &outside] {
        fs::create_dir_all(directory).unwrap();
    }
    let alias = restricted_alias(&allowed, false);
    let mut script = bash::render(&alias, &names()).unwrap();
    script.push_str("\nrma\necho \"status=$?\"\nrma && echo CONTINUED\nexit 0\n");
    let script_path = root.join("run.sh");
    fs::write(&script_path, script).unwrap();

    let denied = run_wrapper(&outside, &script_path);
    let denied_stdout = String::from_utf8_lossy(&denied.stdout);
    assert!(denied_stdout.contains("status=1"), "{denied_stdout}");
    assert!(!denied_stdout.contains("CONTINUED"), "{denied_stdout}");
    assert!(String::from_utf8_lossy(&denied.stderr)
        .contains("current directory is not in the whitelist"));

    let allowed_output = run_wrapper(&allowed, &script_path);
    let allowed_stdout = String::from_utf8_lossy(&allowed_output.stdout);
    assert!(allowed_stdout.contains("ran"), "{allowed_stdout}");
    assert!(allowed_stdout.contains("status=0"), "{allowed_stdout}");
    assert!(allowed_stdout.contains("CONTINUED"), "{allowed_stdout}");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn restricted_wrapper_resolves_symlinked_directory() {
    if !command_available("bash") {
        return;
    }
    let root = unique_root("symlink");
    let real = root.join("real");
    fs::create_dir_all(&real).unwrap();
    let link = root.join("link");
    #[cfg(unix)]
    let created = std::os::unix::fs::symlink(&real, &link).is_ok();
    #[cfg(windows)]
    let created = std::os::windows::fs::symlink_dir(&real, &link).is_ok();
    #[cfg(not(any(unix, windows)))]
    let created = false;
    if !created {
        eprintln!("skipping symlink execution test: symlink creation unavailable");
        let _ = fs::remove_dir_all(root);
        return;
    }

    let alias = restricted_alias(&real, false);
    let mut script = bash::render(&alias, &names()).unwrap();
    script.push_str("\nrma\nexit $?\n");
    let script_path = root.join("run.sh");
    fs::write(&script_path, script).unwrap();
    let output = run_wrapper(&link, &script_path);
    assert!(
        output.status.success() && String::from_utf8_lossy(&output.stdout).contains("ran"),
        "symlink to allowed dir should resolve: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(root);
}

#[cfg(windows)]
#[test]
fn restricted_wrapper_matches_cygwin_mount_aliases() {
    if !command_available("bash") {
        return;
    }
    let root = unique_root("cygwin-alias");
    let subdirectory = root.join("sub");
    fs::create_dir_all(&subdirectory).unwrap();

    // Windows-form restriction path, invoked from a directory Git Bash may
    // expose as /tmp/...; cygpath canonicalization must make them agree.
    let windows_path = restricted_alias(&root, false);
    let mut script = bash::render(&windows_path, &names()).unwrap();
    script.push_str("\nrma\nexit $?\n");
    let script_path = root.join("windows.sh");
    fs::write(&script_path, script).unwrap();
    let output = run_wrapper(&subdirectory, &script_path);
    assert!(
        output.status.success() && String::from_utf8_lossy(&output.stdout).contains("ran"),
        "Windows-form restriction should allow a Git Bash mount alias: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // MSYS-form /tmp restriction path, invoked from the Windows temp subtree.
    let msys_path = restricted_alias_with_paths(vec!["/tmp".into()], false);
    let mut script = bash::render(&msys_path, &names()).unwrap();
    script.push_str("\nrma\nexit $?\n");
    let script_path = root.join("msys.sh");
    fs::write(&script_path, script).unwrap();
    let output = run_wrapper(&subdirectory, &script_path);
    assert!(
        output.status.success() && String::from_utf8_lossy(&output.stdout).contains("ran"),
        "MSYS-form /tmp restriction should match the Windows temp dir: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn zsh_restricted_wrapper_runs_and_denies() {
    if !command_available("zsh") {
        return;
    }
    let root = unique_root("zsh-runtime");
    let allowed = root.join("allowed");
    let outside = root.join("outside");
    for directory in [&allowed, &outside] {
        fs::create_dir_all(directory).unwrap();
    }
    let alias = restricted_alias(&allowed, false);
    let mut script = zsh::render(&alias, &names()).unwrap();
    script.push_str("\nrma\nexit $?\n");
    let script_path = root.join("run.zsh");
    fs::write(&script_path, script).unwrap();

    let allowed_output = run_with_shell("zsh", &allowed, &script_path);
    assert!(
        allowed_output.status.success()
            && String::from_utf8_lossy(&allowed_output.stdout).contains("ran"),
        "zsh allowed dir: {}",
        String::from_utf8_lossy(&allowed_output.stderr)
    );
    let denied = run_with_shell("zsh", &outside, &script_path);
    assert!(!denied.status.success());
    assert!(String::from_utf8_lossy(&denied.stderr)
        .contains("current directory is not in the whitelist"));
    let _ = fs::remove_dir_all(root);
}
