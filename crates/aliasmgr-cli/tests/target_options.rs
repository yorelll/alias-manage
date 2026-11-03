use aliasmgr_cli::cli::{AddArgs, Cli, Command, PathModeArg, TargetTypeArg, UpdateArgs};
use aliasmgr_cli::commands;
use aliasmgr_core::model::{PathMode, TargetType};
use clap::Parser;

fn add_args(name: &str) -> AddArgs {
    AddArgs {
        name: name.into(),
        exec: "git".into(),
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
        no_sync: true,
        allow_missing_loader: true,
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
        no_sync: true,
        allow_missing_loader: true,
    }
}

fn root(label: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "aliasmgr-target-options-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or_default()
    ))
}

#[test]
fn cli_parses_supported_target_types_and_path_modes() {
    let cli = Cli::try_parse_from([
        "aliasmgr",
        "add",
        "gs",
        "--exec",
        "git",
        "--target-type",
        "python_script",
        "--path-mode",
        "relative_to_working_directory",
    ])
    .unwrap();
    let Command::Add(add) = cli.command else {
        panic!("expected add command");
    };
    assert_eq!(add.target_type, Some(TargetTypeArg::PythonScript));
    assert_eq!(add.path_mode, Some(PathModeArg::RelativeToWorkingDirectory));

    let cli = Cli::try_parse_from([
        "aliasmgr",
        "update",
        "gs",
        "--target-type",
        "change_directory",
        "--path-mode",
        "absolute",
    ])
    .unwrap();
    let Command::Update(update) = cli.command else {
        panic!("expected update command");
    };
    assert_eq!(update.target_type, Some(TargetTypeArg::ChangeDirectory));
    assert_eq!(update.path_mode, Some(PathModeArg::Absolute));
}

#[test]
fn cli_rejects_raw_shell_command_and_unknown_values() {
    for value in ["raw_shell_command", "RawShellCommand", "not_a_target"] {
        assert!(
            Cli::try_parse_from([
                "aliasmgr",
                "add",
                "gs",
                "--exec",
                "git",
                "--target-type",
                value,
            ])
            .is_err(),
            "expected {value} to be rejected"
        );
    }
    assert!(Cli::try_parse_from([
        "aliasmgr",
        "add",
        "gs",
        "--exec",
        "git",
        "--path-mode",
        "relative",
    ])
    .is_err());
}

#[test]
fn add_and_update_store_target_type_and_path_mode() {
    let root = root("store");
    let cfg = root.to_string_lossy().to_string();

    let mut add = add_args("jar");
    add.exec = "app.jar".into();
    add.target_type = Some(TargetTypeArg::JavaJar);
    add.path_mode = Some(PathModeArg::RelativeToWorkingDirectory);
    let alias = commands::add(Some(&cfg), &add).unwrap();
    assert_eq!(alias.target_type, TargetType::JavaJar);
    assert_eq!(alias.path_mode, PathMode::RelativeToWorkingDirectory);
    let stored = commands::get(Some(&cfg), "jar").unwrap();
    assert_eq!(stored.target_type, TargetType::JavaJar);
    assert_eq!(stored.path_mode, PathMode::RelativeToWorkingDirectory);

    let mut update = update_args("jar");
    update.target_type = Some(TargetTypeArg::Batch);
    update.path_mode = Some(PathModeArg::Absolute);
    let updated = commands::update(Some(&cfg), &update).unwrap();
    assert_eq!(updated.target_type, TargetType::Batch);
    assert_eq!(updated.path_mode, PathMode::Absolute);

    // Omitting the flags keeps the stored values.
    let mut noop = update_args("jar");
    noop.exec = Some("app.jar".into());
    let unchanged = commands::update(Some(&cfg), &noop).unwrap();
    assert_eq!(unchanged.target_type, TargetType::Batch);
    assert_eq!(unchanged.path_mode, PathMode::Absolute);

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn every_exposed_target_type_maps_to_the_core_enum() {
    let cases = [
        (
            TargetTypeArg::NativeExecutable,
            TargetType::NativeExecutable,
        ),
        (TargetTypeArg::PythonScript, TargetType::PythonScript),
        (
            TargetTypeArg::PowerShellScript,
            TargetType::PowerShellScript,
        ),
        (TargetTypeArg::Batch, TargetType::Batch),
        (TargetTypeArg::ShellScript, TargetType::ShellScript),
        (TargetTypeArg::JavaJar, TargetType::JavaJar),
        (TargetTypeArg::Cmdlet, TargetType::Cmdlet),
        (TargetTypeArg::ChangeDirectory, TargetType::ChangeDirectory),
    ];
    for (argument, expected) in cases {
        assert_eq!(TargetType::from(argument), expected);
    }
}
