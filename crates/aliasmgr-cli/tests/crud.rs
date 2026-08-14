use aliasmgr_cli::commands;
use aliasmgr_cli::cli::AddArgs;

#[test]
fn cli_crud_lifecycle_uses_isolated_database() {
    let root = std::env::temp_dir().join(format!("aliasmgr-cli-crud-{}", std::process::id()));
    let root_string = root.to_string_lossy().to_string();
    let args = AddArgs { name: "gs".into(), exec: "git".into(), args: vec!["status".into()], shell: vec!["bash".into()], cwd: None, environment: vec![], tags: vec!["git".into()], no_pass_args: false };
    commands::add(Some(&root_string), &args).unwrap();
    assert_eq!(commands::get(Some(&root_string), "gs").unwrap().unwrap().fixed_args, vec!["status"]);
    assert!(commands::enable(Some(&root_string), "gs", false).unwrap());
    assert!(!commands::get(Some(&root_string), "gs").unwrap().unwrap().enabled);
    assert!(commands::enable(Some(&root_string), "gs", true).unwrap());
    assert!(commands::remove(Some(&root_string), "gs").unwrap());
    assert!(commands::get(Some(&root_string), "gs").unwrap().is_none());
    let _ = std::fs::remove_dir_all(root);
}
