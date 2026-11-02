use aliasmgr_cli::cli::{AddArgs, RestrictionModeArg, UpdateArgs};
use aliasmgr_cli::commands;
use aliasmgr_core::error::AliasError;
use aliasmgr_core::model::{PathRestriction, RestrictionMode};

fn args(name: &str) -> AddArgs {
    AddArgs {
        name: name.into(),
        exec: "git".into(),
        args: vec!["status".into()],
        shell: vec!["bash".into()],
        cwd: None,
        environment: vec![],
        tags: vec!["git".into()],
        description: None,
        restrict: None,
        restrict_paths: vec![],
        clear_restriction: false,
        no_pass_args: false,
        target_type: None,
        path_mode: None,
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
    }
}

#[test]
fn cli_lifecycle_supports_update_rename_find_and_retirement() {
    let root = std::env::temp_dir().join(format!("aliasmgr-cli-crud-{}", std::process::id()));
    let cfg = root.to_string_lossy().to_string();
    commands::add(Some(&cfg), &args("gs")).unwrap();
    let mut change = update_args("gs");
    change.args = Some(vec!["diff".into()]);
    let updated = commands::update(Some(&cfg), &change).unwrap();
    assert_eq!(updated.fixed_args, vec!["diff"]);
    assert!(commands::rename(Some(&cfg), "gs", "gd").unwrap());
    assert_eq!(commands::get(Some(&cfg), "gd").unwrap().name, "gd");
    assert!(commands::find(Some(&cfg), "gd", false, None, Vec::new())
        .unwrap()
        .iter()
        .any(|a| a.name == "gd"));
    assert!(commands::retired_names(Some(&cfg), "bash")
        .unwrap()
        .contains(&"gs".into()));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn update_preserves_unspecified_fields_and_replaces_supplied_args() {
    let root = std::env::temp_dir().join(format!("aliasmgr-cli-update-{}", std::process::id()));
    let cfg = root.to_string_lossy().to_string();
    commands::add(Some(&cfg), &args("gs")).unwrap();

    let mut change = update_args("gs");
    change.exec = Some("git".into());
    let updated = commands::update(Some(&cfg), &change).unwrap();
    assert_eq!(updated.executable, "git");
    assert_eq!(updated.fixed_args, vec!["status"]);
    assert_eq!(updated.tags, vec!["git"]);
    assert!(updated.pass_args);
    assert_eq!(
        commands::get(Some(&cfg), "gs").unwrap().fixed_args,
        vec!["status"]
    );

    let mut replace = update_args("gs");
    replace.args = Some(vec!["diff".into(), "--stat".into()]);
    let updated = commands::update(Some(&cfg), &replace).unwrap();
    assert_eq!(updated.fixed_args, vec!["diff", "--stat"]);
    assert_eq!(
        commands::get(Some(&cfg), "gs").unwrap().fixed_args,
        vec!["diff", "--stat"]
    );

    let mut disable_pass = update_args("gs");
    disable_pass.no_pass_args = true;
    assert!(
        !commands::update(Some(&cfg), &disable_pass)
            .unwrap()
            .pass_args
    );
    let mut enable_pass = update_args("gs");
    enable_pass.pass_args = true;
    assert!(
        commands::update(Some(&cfg), &enable_pass)
            .unwrap()
            .pass_args
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn description_round_trips_and_empty_string_clears_it() {
    let root =
        std::env::temp_dir().join(format!("aliasmgr-cli-description-{}", std::process::id()));
    let cfg = root.to_string_lossy().to_string();
    let mut add = args("desc");
    add.description = Some("first memo".into());
    assert_eq!(
        commands::add(Some(&cfg), &add).unwrap().description,
        "first memo"
    );
    let mut clear = update_args("desc");
    clear.description = Some(String::new());
    assert_eq!(
        commands::update(Some(&cfg), &clear).unwrap().description,
        ""
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn restriction_flags_follow_add_and_update_semantics() {
    let root =
        std::env::temp_dir().join(format!("aliasmgr-cli-restriction-{}", std::process::id()));
    let cfg = root.to_string_lossy().to_string();
    let base = std::env::temp_dir().to_string_lossy().into_owned();
    let first = format!("{base}/restriction-one");
    let second = format!("{base}/restriction-two");

    let mut add = args("rma");
    add.restrict = Some(RestrictionModeArg::Whitelist);
    add.restrict_paths = vec![format!(" {first} "), second.clone()];
    let created = commands::add(Some(&cfg), &add).unwrap();
    assert_eq!(
        created.restriction,
        Some(PathRestriction {
            mode: RestrictionMode::Whitelist,
            paths: vec![first.clone(), second.clone()],
        })
    );

    let mut missing_paths = args("missing-paths");
    missing_paths.restrict = Some(RestrictionModeArg::Whitelist);
    assert!(matches!(
        commands::add(Some(&cfg), &missing_paths),
        Err(AliasError::Config(message)) if message.contains("--restrict-path")
    ));

    let mut path_only = args("path-only");
    path_only.restrict_paths = vec![first.clone()];
    assert!(matches!(
        commands::add(Some(&cfg), &path_only),
        Err(AliasError::Config(message)) if message.contains("--restrict")
    ));

    let mut mode_only = update_args("rma");
    mode_only.restrict = Some(RestrictionModeArg::Blacklist);
    let changed = commands::update(Some(&cfg), &mode_only).unwrap();
    assert_eq!(
        changed.restriction,
        Some(PathRestriction {
            mode: RestrictionMode::Blacklist,
            paths: vec![first.clone(), second.clone()],
        })
    );

    let mut replace_paths = update_args("rma");
    replace_paths.restrict_paths = vec![second.clone()];
    let changed = commands::update(Some(&cfg), &replace_paths).unwrap();
    assert_eq!(
        changed.restriction,
        Some(PathRestriction {
            mode: RestrictionMode::Blacklist,
            paths: vec![second.clone()],
        })
    );

    let mut clear = update_args("rma");
    clear.clear_restriction = true;
    assert_eq!(
        commands::update(Some(&cfg), &clear).unwrap().restriction,
        None
    );

    commands::add(Some(&cfg), &args("plain")).unwrap();
    let mut restrict_only = update_args("plain");
    restrict_only.restrict = Some(RestrictionModeArg::Whitelist);
    assert!(matches!(
        commands::update(Some(&cfg), &restrict_only),
        Err(AliasError::Config(message)) if message.contains("no restriction exists")
    ));
    let mut path_only_update = update_args("plain");
    path_only_update.restrict_paths = vec![first.clone()];
    assert!(matches!(
        commands::update(Some(&cfg), &path_only_update),
        Err(AliasError::Config(message)) if message.contains("existing restriction")
    ));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn update_merges_environment_and_rejects_invalid_shells_and_entries() {
    let root = std::env::temp_dir().join(format!("aliasmgr-cli-update-env-{}", std::process::id()));
    let cfg = root.to_string_lossy().to_string();
    let mut add = args("gs");
    add.environment = vec!["MODE=test".into(), "KEEP=1".into()];
    let created = commands::add(Some(&cfg), &add).unwrap();
    assert_eq!(
        created.environment.get("MODE").map(String::as_str),
        Some("test")
    );

    let mut change = update_args("gs");
    change.environment = Some(vec!["MODE=prod".into(), "NEW=2".into()]);
    let updated = commands::update(Some(&cfg), &change).unwrap();
    assert_eq!(
        updated.environment.get("MODE").map(String::as_str),
        Some("prod")
    );
    assert_eq!(
        updated.environment.get("KEEP").map(String::as_str),
        Some("1")
    );
    assert_eq!(
        updated.environment.get("NEW").map(String::as_str),
        Some("2")
    );

    let mut broken = update_args("gs");
    broken.environment = Some(vec!["MISSING".into()]);
    assert!(
        matches!(commands::update(Some(&cfg), &broken), Err(AliasError::Config(message)) if message.contains("KEY=VALUE"))
    );

    let mut bad_shell = update_args("gs");
    bad_shell.shell = Some(vec!["tcsh".into()]);
    assert!(
        matches!(commands::update(Some(&cfg), &bad_shell), Err(AliasError::Config(message)) if message.contains("unsupported shell"))
    );

    let mut bad_add_shell = args("bad");
    bad_add_shell.shell = vec!["tcsh".into()];
    assert!(
        matches!(commands::add(Some(&cfg), &bad_add_shell), Err(AliasError::Config(message)) if message.contains("unsupported shell"))
    );

    let mut bad_add_env = args("badenv");
    bad_add_env.environment = vec!["NOEQUALS".into()];
    assert!(
        matches!(commands::add(Some(&cfg), &bad_add_env), Err(AliasError::Config(message)) if message.contains("KEY=VALUE"))
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn cli_query_propagates_field_sort_descending_limit_and_tags() {
    let root = std::env::temp_dir().join(format!("aliasmgr-cli-query-{}", std::process::id()));
    let cfg = root.to_string_lossy().to_string();
    let mut first = args("alpha");
    first.tags = vec!["work".into(), "python".into()];
    commands::add(Some(&cfg), &first).unwrap();
    let mut second = args("beta");
    second.tags = vec!["work".into()];
    commands::add(Some(&cfg), &second).unwrap();

    let listed = commands::list_query(
        Some(&cfg),
        Some("name"),
        true,
        Some(1),
        vec!["work".into(), "python".into()],
    )
    .unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].name, "alpha");
    let found = commands::find_query(
        Some(&cfg),
        "git",
        false,
        Some("target"),
        None,
        vec!["python".into()],
    )
    .unwrap();
    assert_eq!(
        found
            .iter()
            .map(|alias| alias.name.as_str())
            .collect::<Vec<_>>(),
        vec!["alpha"]
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn cli_json_rows_include_complete_alias_fields() {
    let root = std::env::temp_dir().join(format!("aliasmgr-cli-json-{}", std::process::id()));
    let cfg = root.to_string_lossy().to_string();
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

#[test]
fn update_shell_retires_removed_shell_and_sync_cleans_it() {
    let root =
        std::env::temp_dir().join(format!("aliasmgr-cli-update-shell-{}", std::process::id()));
    let cfg = root.to_string_lossy().to_string();
    commands::add(Some(&cfg), &args("gs")).unwrap();
    commands::sync(Some(&cfg), false).unwrap();
    let bash = root.join("generated").join("bash.sh");
    let content = std::fs::read_to_string(&bash).unwrap();
    assert!(content.contains("gs() {"));

    let mut change = update_args("gs");
    change.shell = Some(vec!["zsh".into()]);
    commands::update(Some(&cfg), &change).unwrap();
    commands::sync(Some(&cfg), false).unwrap();
    let content = std::fs::read_to_string(&bash).unwrap();
    assert!(!content.contains("gs() {"));
    assert!(content.contains("# retired: gs"));
    let zsh = root.join("generated").join("zsh.sh");
    assert!(std::fs::read_to_string(&zsh).unwrap().contains("gs() {"));

    assert!(commands::remove(Some(&cfg), "gs").unwrap());
    commands::sync(Some(&cfg), false).unwrap();
    let zsh = std::fs::read_to_string(&zsh).unwrap();
    assert!(!zsh.contains("gs() {"));
    assert!(zsh.contains("# retired: gs"));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn missing_aliases_report_not_found_for_lifecycle_commands() {
    let root = std::env::temp_dir().join(format!("aliasmgr-cli-missing-{}", std::process::id()));
    let cfg = root.to_string_lossy().to_string();
    assert!(
        matches!(commands::get(Some(&cfg), "missing"), Err(AliasError::NotFound(name)) if name == "missing")
    );
    assert!(
        matches!(commands::remove(Some(&cfg), "missing"), Err(AliasError::NotFound(name)) if name == "missing")
    );
    assert!(
        matches!(commands::update(Some(&cfg), &update_args("missing")), Err(AliasError::NotFound(name)) if name == "missing")
    );
    assert!(
        matches!(commands::rename(Some(&cfg), "missing", "other"), Err(AliasError::NotFound(name)) if name == "missing")
    );
    assert!(matches!(
        commands::enable(Some(&cfg), "missing", true),
        Err(AliasError::NotFound(_))
    ));
    assert!(matches!(
        commands::enable(Some(&cfg), "missing", false),
        Err(AliasError::NotFound(_))
    ));
    let _ = std::fs::remove_dir_all(root);
}
