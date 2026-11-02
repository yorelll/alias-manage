use aliasmgr_cli::cli::{Cli, Command, RestrictionModeArg, ShellCommand};
use clap::Parser;

#[test]
fn parses_add_with_structured_arguments() {
    let cli = Cli::try_parse_from([
        "aliasmgr",
        "add",
        "cm",
        "--exec",
        "python3",
        "--arg",
        "copymv.py",
        "--shell",
        "bash",
    ])
    .unwrap();
    match cli.command {
        Command::Add(args) => {
            assert_eq!(args.name, "cm");
            assert_eq!(args.args, vec!["copymv.py"]);
            assert_eq!(args.shell, vec!["bash"]);
        }
        _ => panic!("expected add"),
    }
}

#[test]
fn parses_leading_hyphen_arguments_as_values_for_add_and_update() {
    let cli = Cli::try_parse_from([
        "aliasmgr",
        "add",
        "gl",
        "--exec",
        "git",
        "--arg",
        "status",
        "--arg",
        "--oneline",
    ])
    .unwrap();
    match cli.command {
        Command::Add(args) => assert_eq!(args.args, vec!["status", "--oneline"]),
        _ => panic!("expected add"),
    }

    let cli = Cli::try_parse_from(["aliasmgr", "update", "gl", "--arg", "--oneline"]).unwrap();
    match cli.command {
        Command::Update(args) => assert_eq!(args.args, Some(vec!["--oneline".to_string()])),
        _ => panic!("expected update"),
    }
}

#[test]
fn parses_update_optional_fields_and_conflicting_pass_args() {
    let cli = Cli::try_parse_from([
        "aliasmgr",
        "update",
        "gs",
        "--exec",
        "git",
        "--shell",
        "bash",
        "--cwd",
        "/tmp",
        "--env",
        "A=1",
        "--tag",
        "git",
        "--no-pass-args",
    ])
    .unwrap();
    match cli.command {
        Command::Update(args) => {
            assert_eq!(args.name, "gs");
            assert_eq!(args.exec.as_deref(), Some("git"));
            assert_eq!(args.args, None);
            assert_eq!(args.shell, Some(vec!["bash".to_string()]));
            assert_eq!(args.cwd.as_deref(), Some("/tmp"));
            assert_eq!(args.environment, Some(vec!["A=1".to_string()]));
            assert_eq!(args.tags, Some(vec!["git".to_string()]));
            assert!(args.no_pass_args);
            assert!(!args.pass_args);
        }
        _ => panic!("expected update"),
    }

    let cli = Cli::try_parse_from(["aliasmgr", "update", "gs", "--pass-args"]).unwrap();
    match cli.command {
        Command::Update(args) => {
            assert!(args.pass_args);
            assert!(!args.no_pass_args);
        }
        _ => panic!("expected update"),
    }
    assert!(
        Cli::try_parse_from(["aliasmgr", "update", "gs", "--pass-args", "--no-pass-args"]).is_err()
    );
}

#[test]
fn parses_reload_shell_and_import_confirmation() {
    let cli = Cli::try_parse_from(["aliasmgr", "reload", "--print", "--shell", "zsh"]).unwrap();
    match cli.command {
        Command::Reload { print, shell } => {
            assert!(print);
            assert_eq!(shell.as_deref(), Some("zsh"));
        }
        _ => panic!("expected reload"),
    }

    let cli = Cli::try_parse_from(["aliasmgr", "reload", "--print"]).unwrap();
    match cli.command {
        Command::Reload { print, shell } => {
            assert!(print);
            assert_eq!(shell, None);
        }
        _ => panic!("expected reload"),
    }

    let cli = Cli::try_parse_from(["aliasmgr", "import", "file.json", "--yes"]).unwrap();
    match cli.command {
        Command::Import { file, yes } => {
            assert_eq!(file, "file.json");
            assert!(yes);
        }
        _ => panic!("expected import"),
    }

    let cli = Cli::try_parse_from(["aliasmgr", "reload", "--print", "--shell", "ZSH"]).unwrap();
    match cli.command {
        Command::Reload { shell, .. } => assert_eq!(shell.as_deref(), Some("ZSH")),
        _ => panic!("expected reload"),
    }

    let cli = Cli::try_parse_from(["aliasmgr", "uninstall", "--purge-aliases", "--yes"]).unwrap();
    match cli.command {
        Command::Uninstall { purge_aliases, yes } => {
            assert!(purge_aliases);
            assert!(yes);
        }
        _ => panic!("expected uninstall"),
    }
    let cli = Cli::try_parse_from(["aliasmgr", "uninstall", "--purge-aliases"]).unwrap();
    match cli.command {
        Command::Uninstall { yes, .. } => assert!(!yes),
        _ => panic!("expected uninstall"),
    }
}

#[test]
fn parses_description_and_restriction_flags() {
    let cli = Cli::try_parse_from([
        "aliasmgr",
        "add",
        "rma",
        "--exec",
        "rm",
        "--description",
        "Remove build output",
        "--restrict",
        "whitelist",
        "--restrict-path",
        "/srv/project",
        "--restrict-path",
        "/srv/shared",
    ])
    .unwrap();
    match cli.command {
        Command::Add(args) => {
            assert_eq!(args.description.as_deref(), Some("Remove build output"));
            assert!(matches!(args.restrict, Some(RestrictionModeArg::Whitelist)));
            assert_eq!(args.restrict_paths, vec!["/srv/project", "/srv/shared"]);
        }
        _ => panic!("expected add"),
    }

    assert!(
        Cli::try_parse_from([
            "aliasmgr",
            "add",
            "rma",
            "--exec",
            "rm",
            "--clear-restriction"
        ])
        .is_err(),
        "add must not accept --clear-restriction"
    );

    let cli = Cli::try_parse_from([
        "aliasmgr",
        "update",
        "rma",
        "--restrict",
        "blacklist",
        "--clear-restriction",
    ]);
    assert!(
        cli.is_err(),
        "--clear-restriction must conflict with --restrict"
    );
    let cli = Cli::try_parse_from([
        "aliasmgr",
        "update",
        "rma",
        "--restrict-path",
        "/srv",
        "--clear-restriction",
    ]);
    assert!(
        cli.is_err(),
        "--clear-restriction must conflict with --restrict-path"
    );

    let cli = Cli::try_parse_from(["aliasmgr", "update", "rma", "--clear-restriction"]).unwrap();
    match cli.command {
        Command::Update(args) => {
            assert!(args.clear_restriction);
            assert!(args.restrict.is_none());
            assert!(args.restrict_paths.is_empty());
        }
        _ => panic!("expected update"),
    }
}

#[test]
fn parses_lifecycle_and_diagnostic_commands() {
    for argv in [
        vec!["aliasmgr", "remove", "cm", "--yes"],
        vec!["aliasmgr", "find", "query", "--fuzzy", "--limit", "10"],
        vec!["aliasmgr", "list", "--sort", "updated_at", "--desc"],
        vec!["aliasmgr", "sync", "--dry-run"],
        vec!["aliasmgr", "reload", "--print"],
        vec!["aliasmgr", "doctor"],
        vec!["aliasmgr", "import", "file.json"],
        vec!["aliasmgr", "export", "file.json"],
        vec!["aliasmgr", "uninstall", "--purge-aliases"],
    ] {
        Cli::try_parse_from(argv).unwrap();
    }
    let cli = Cli::try_parse_from(["aliasmgr", "shell", "detect"]).unwrap();
    assert!(matches!(
        cli.command,
        Command::Shell {
            command: ShellCommand::Detect
        }
    ));
}

#[test]
fn global_options_parse() {
    let cli = Cli::try_parse_from([
        "aliasmgr",
        "--config-dir",
        "/tmp/test",
        "--format",
        "json",
        "--quiet",
        "list",
    ])
    .unwrap();
    assert_eq!(cli.config_dir.as_deref(), Some("/tmp/test"));
    assert!(cli.quiet);
}

#[test]
fn parses_complete_search_and_list_query_options() {
    let cli = Cli::try_parse_from([
        "aliasmgr",
        "find",
        "build",
        "--fuzzy",
        "--field",
        "description",
        "--tag",
        "work",
        "--tag",
        "python",
        "--limit",
        "7",
    ])
    .unwrap();
    match cli.command {
        Command::Find {
            query,
            fuzzy,
            field,
            limit,
            tags,
        } => {
            assert_eq!(query, "build");
            assert!(fuzzy);
            assert_eq!(field.as_deref(), Some("description"));
            assert_eq!(limit, Some(7));
            assert_eq!(tags, vec!["work", "python"]);
        }
        _ => panic!("expected find"),
    }

    let cli = Cli::try_parse_from([
        "aliasmgr",
        "--format",
        "json",
        "list",
        "--sort",
        "updated_at",
        "--desc",
        "--tag",
        "work",
        "--tag",
        "python",
        "--limit",
        "0",
    ])
    .unwrap();
    match cli.command {
        Command::List {
            sort,
            desc,
            limit,
            tags,
        } => {
            assert_eq!(sort.as_deref(), Some("updated_at"));
            assert!(desc);
            assert_eq!(limit, Some(0));
            assert_eq!(tags, vec!["work", "python"]);
        }
        _ => panic!("expected list"),
    }
}
