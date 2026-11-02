use aliasmgr_cli::cli::AddArgs;
use aliasmgr_cli::commands;
use aliasmgr_core::config::{AppConfig, AppPaths};
use aliasmgr_core::shells::common::{END_MARKER, START_MARKER};
use aliasmgr_core::uninstall::{uninstall, UninstallMode};

/// Serializes tests that temporarily override the platform home directory.
static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Isolates `%USERPROFILE%` / `$HOME` so default PowerShell profile resolution
/// in tests can never visit (or clean) the developer's real profile.
struct HomeGuard {
    path: std::path::PathBuf,
    userprofile: Option<std::ffi::OsString>,
    home: Option<std::ffi::OsString>,
}

impl HomeGuard {
    fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "aliasmgr-cli-test-home-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or_default()
        ));
        std::fs::create_dir_all(&path).unwrap();
        let userprofile = std::env::var_os("USERPROFILE");
        let home = std::env::var_os("HOME");
        std::env::set_var("USERPROFILE", &path);
        std::env::set_var("HOME", &path);
        Self {
            path,
            userprofile,
            home,
        }
    }
}

impl Drop for HomeGuard {
    fn drop(&mut self) {
        match self.userprofile.take() {
            Some(value) => std::env::set_var("USERPROFILE", value),
            None => std::env::remove_var("USERPROFILE"),
        }
        match self.home.take() {
            Some(value) => std::env::set_var("HOME", value),
            None => std::env::remove_var("HOME"),
        }
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

#[test]
fn uninstall_modes_preserve_referenced_targets() {
    let _env = ENV_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let _home = HomeGuard::new("modes");
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
    let _env = ENV_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let _home = HomeGuard::new("loader");
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
fn cli_uninstall_removes_a_loader_installed_at_the_default_powershell_profile() {
    let _env = ENV_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let home = HomeGuard::new("default-profile");
    let root = std::env::temp_dir().join(format!(
        "aliasmgr-cli-uninstall-default-profile-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).unwrap();
    AppConfig::default()
        .save(&AppPaths { root: root.clone() })
        .unwrap();
    let config_dir = root.to_string_lossy().to_string();

    // No configured profile path: install and uninstall must agree on the
    // platform default resolved from the isolated home.
    commands::shell_install(Some(&config_dir), "powershell7").unwrap();
    let profile = aliasmgr_core::integration::default_powershell_profile_path("powershell7")
        .expect("PowerShell 7 has a default profile on this platform");
    assert!(profile.exists());
    assert!(std::fs::read_to_string(&profile)
        .unwrap()
        .contains(START_MARKER));

    commands::uninstall(Some(&config_dir), false).unwrap();
    let content = std::fs::read_to_string(&profile).unwrap_or_default();
    assert!(
        !content.contains(START_MARKER),
        "CLI uninstall must remove the default-profile loader: {content}"
    );
    assert!(
        !home.path.join(".bashrc").exists()
            || !std::fs::read_to_string(home.path.join(".bashrc"))
                .unwrap()
                .contains(START_MARKER)
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn cli_uninstall_removes_configured_and_root_fallback_bash_loaders() {
    let _env = ENV_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let _home = HomeGuard::new("override-fallback");
    let root = std::env::temp_dir().join(format!(
        "aliasmgr-cli-uninstall-override-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(root.join("generated")).unwrap();
    let configured = root.join("custom.bashrc");
    std::fs::write(&configured, "configured user\n").unwrap();
    let fallback = root.join(".bashrc");
    std::fs::write(&fallback, "fallback user\n").unwrap();
    let loader = aliasmgr_core::shells::common::render_loader(&root.join("generated/bash.sh"));
    for path in [&configured, &fallback] {
        let content = aliasmgr_core::shells::common::install_loader(
            &std::fs::read_to_string(path).unwrap(),
            &loader,
        )
        .unwrap();
        std::fs::write(path, content).unwrap();
    }
    let mut config = AppConfig::default();
    config.shells.bash_rc_path = Some(configured.clone());
    config.save(&AppPaths { root: root.clone() }).unwrap();

    commands::uninstall(Some(&root.to_string_lossy()), false).unwrap();
    for (path, expected) in [
        (&configured, "configured user\n"),
        (&fallback, "fallback user\n"),
    ] {
        let content = std::fs::read_to_string(path).unwrap();
        assert!(
            !content.contains(START_MARKER) && !content.contains(END_MARKER),
            "loader survived in {}: {content}",
            path.display()
        );
        assert!(content.contains(expected.trim()));
    }
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn uninstall_purges_even_when_config_is_unparsable() {
    let _env = ENV_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let _home = HomeGuard::new("badcfg");
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
