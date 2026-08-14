use aliasmgr_cli::commands;
use aliasmgr_cli::cli::AddArgs;

fn args(name: &str) -> AddArgs { AddArgs { name: name.into(), exec: "git".into(), args: vec!["status".into()], shell: vec!["bash".into()], cwd: None, environment: vec![], tags: vec!["git".into()], no_pass_args: false } }

#[test]
fn cli_lifecycle_supports_update_rename_find_and_retirement() {
    let root = std::env::temp_dir().join(format!("aliasmgr-cli-crud-{}", std::process::id())); let cfg = root.to_string_lossy().to_string();
    commands::add(Some(&cfg), &args("gs")).unwrap();
    assert!(commands::update(Some(&cfg), "gs", "git", vec!["diff".into()]).unwrap());
    assert!(commands::rename(Some(&cfg), "gs", "gd").unwrap());
    assert!(commands::get(Some(&cfg), "gd").unwrap().is_some());
    assert!(commands::find(Some(&cfg), "gd").unwrap().iter().any(|a| a.name == "gd"));
    assert!(commands::retired_names(Some(&cfg), "bash").unwrap().contains(&"gs".into()));
    let _ = std::fs::remove_dir_all(root);
}
