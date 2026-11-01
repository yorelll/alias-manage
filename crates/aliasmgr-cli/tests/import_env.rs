use aliasmgr_cli::cli::AddArgs;
use aliasmgr_cli::commands;
use aliasmgr_core::{model::AliasRecord, transfer::export_json};

// Environment variables are process-global, so this file deliberately holds a
// single test that temporarily overrides ALIASMGR_CONFIG_DIR.
#[test]
fn preview_and_confirm_agree_when_only_the_environment_config_dir_is_set() {
    let root = std::env::temp_dir().join(format!("aliasmgr-cli-env-import-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let previous = std::env::var_os("ALIASMGR_CONFIG_DIR");
    std::env::set_var("ALIASMGR_CONFIG_DIR", &root);

    let add = AddArgs {
        name: "existing".into(),
        exec: "git".into(),
        args: vec![],
        shell: vec!["bash".into()],
        cwd: None,
        environment: vec![],
        tags: vec![],
        no_pass_args: false,
    };
    commands::add(None, &add).unwrap();

    let file = root.join("incoming.json");
    let incoming = AliasRecord {
        name: "existing".into(),
        executable: "git".into(),
        ..Default::default()
    };
    export_json(&file, std::slice::from_ref(&incoming)).unwrap();

    let preview = commands::import_file(None, file.to_str().unwrap(), false).unwrap();
    assert!(preview.imported.is_empty());
    assert_eq!(preview.skipped, vec!["existing"]);
    assert!(!root.join("backups").join("db").exists());

    let confirmed = commands::import_file(None, file.to_str().unwrap(), true).unwrap();
    assert!(confirmed.imported.is_empty());
    assert_eq!(confirmed.skipped, vec!["existing"]);

    match previous {
        Some(value) => std::env::set_var("ALIASMGR_CONFIG_DIR", value),
        None => std::env::remove_var("ALIASMGR_CONFIG_DIR"),
    }
    let _ = std::fs::remove_dir_all(root);
}
