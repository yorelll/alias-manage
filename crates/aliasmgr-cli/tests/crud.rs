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
    assert!(commands::find(Some(&cfg), "gd", false, None, Vec::new()).unwrap().iter().any(|a| a.name == "gd"));
    assert!(commands::retired_names(Some(&cfg), "bash").unwrap().contains(&"gs".into()));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn cli_query_propagates_field_sort_descending_limit_and_tags() {
    let root = std::env::temp_dir().join(format!("aliasmgr-cli-query-{}", std::process::id())); let cfg = root.to_string_lossy().to_string();
    let mut first = args("alpha");
    first.tags = vec!["work".into(), "python".into()];
    commands::add(Some(&cfg), &first).unwrap();
    let mut second = args("beta");
    second.tags = vec!["work".into()];
    commands::add(Some(&cfg), &second).unwrap();

    let listed = commands::list_query(Some(&cfg), Some("name"), true, Some(1), vec!["work".into(), "python".into()]).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].name, "alpha");
    let found = commands::find_query(Some(&cfg), "git", false, Some("target"), None, vec!["python".into()]).unwrap();
    assert_eq!(found.iter().map(|alias| alias.name.as_str()).collect::<Vec<_>>(), vec!["alpha"]);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn cli_json_rows_include_complete_alias_fields() {
    let root = std::env::temp_dir().join(format!("aliasmgr-cli-json-{}", std::process::id())); let cfg = root.to_string_lossy().to_string();
    let mut add = args("gs");
    add.tags = vec!["git".into()];
    commands::add(Some(&cfg), &add).unwrap();
    let aliases = commands::list_query(Some(&cfg), None, false, None, Vec::new()).unwrap();
    let output = aliasmgr_cli::output::alias_rows(&aliases, aliasmgr_cli::cli::OutputFormat::Json);
    assert!(output.contains("\"fixed_args\""));
    assert!(output.contains("\"shells\""));
    assert!(output.contains("\"tags\""));
    let _ = std::fs::remove_dir_all(root);
}
