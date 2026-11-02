use aliasmgr_cli::cli::AddArgs;
use aliasmgr_cli::commands;
use std::sync::Mutex;

// Environment variables and the process working directory are process-global,
// so every test in this binary runs under the same lock.
static ENV_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn environment_root_drives_add_sync_and_reload_without_local_fallback() {
    let _guard = ENV_LOCK.lock().unwrap();
    let base = std::env::temp_dir().join(format!("aliasmgr-cli-root-{}", std::process::id()));
    let config_root = base.join("config");
    let sandbox = base.join("cwd");
    std::fs::create_dir_all(&config_root).unwrap();
    std::fs::create_dir_all(&sandbox).unwrap();

    let previous_dir = std::env::current_dir().unwrap();
    let previous_env = std::env::var_os("ALIASMGR_CONFIG_DIR");
    std::env::set_var("ALIASMGR_CONFIG_DIR", &config_root);
    std::env::set_current_dir(&sandbox).unwrap();

    let outcome = std::panic::catch_unwind(|| {
        let add = AddArgs {
            name: "gs".into(),
            exec: "git".into(),
            args: vec!["status".into()],
            shell: vec!["bash".into()],
            cwd: None,
            environment: vec![],
            tags: vec![],
            description: None,
            restrict: None,
            restrict_paths: vec![],
            clear_restriction: false,
            no_pass_args: false,
            target_type: None,
            path_mode: None,
        };
        commands::add(None, &add).unwrap();
        assert!(config_root.join("aliases.db").exists());

        commands::sync(None, false).unwrap();
        assert!(config_root.join("generated/bash.sh").exists());
        assert!(!sandbox.join(".alias-manager").exists());

        let reload = commands::reload_print(None, "bash");
        assert!(
            reload.contains("generated/bash.sh"),
            "reload output: {reload}"
        );
        assert!(
            reload.contains(config_root.to_string_lossy().as_ref()),
            "reload output: {reload}"
        );
    });

    std::env::set_current_dir(previous_dir).unwrap();
    match previous_env {
        Some(value) => std::env::set_var("ALIASMGR_CONFIG_DIR", value),
        None => std::env::remove_var("ALIASMGR_CONFIG_DIR"),
    }
    let _ = std::fs::remove_dir_all(&base);
    if let Err(payload) = outcome {
        std::panic::resume_unwind(payload);
    }
}
