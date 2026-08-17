use aliasmgr_cli::cli::{Cli, Command, ShellCommand};
use clap::Parser;

#[test]
fn parses_add_with_structured_arguments() {
    let cli = Cli::try_parse_from(["aliasmgr", "add", "cm", "--exec", "python3", "--arg", "copymv.py", "--shell", "bash"]).unwrap();
    match cli.command { Command::Add(args) => { assert_eq!(args.name, "cm"); assert_eq!(args.args, vec!["copymv.py"]); assert_eq!(args.shell, vec!["bash"]); }, _ => panic!("expected add") }
}

#[test]
fn parses_lifecycle_and_diagnostic_commands() {
    for argv in [
        vec!["aliasmgr", "remove", "cm", "--yes"], vec!["aliasmgr", "find", "query", "--fuzzy", "--limit", "10"],
        vec!["aliasmgr", "list", "--sort", "updated_at", "--desc"], vec!["aliasmgr", "sync", "--dry-run"],
        vec!["aliasmgr", "reload", "--print"], vec!["aliasmgr", "doctor"], vec!["aliasmgr", "import", "file.json"],
        vec!["aliasmgr", "export", "file.json"], vec!["aliasmgr", "uninstall", "--purge-aliases"],
    ] { Cli::try_parse_from(argv).unwrap(); }
    let cli = Cli::try_parse_from(["aliasmgr", "shell", "detect"]).unwrap();
    assert!(matches!(cli.command, Command::Shell { command: ShellCommand::Detect }));
}

#[test]
fn global_options_parse() {
    let cli = Cli::try_parse_from(["aliasmgr", "--config-dir", "/tmp/test", "--format", "json", "--quiet", "list"]).unwrap();
    assert_eq!(cli.config_dir.as_deref(), Some("/tmp/test"));
    assert!(cli.quiet);
}

#[test]
fn parses_complete_search_and_list_query_options() {
    let cli = Cli::try_parse_from([
        "aliasmgr", "find", "build", "--fuzzy", "--field", "description",
        "--tag", "work", "--tag", "python", "--limit", "7",
    ]).unwrap();
    match cli.command {
        Command::Find { query, fuzzy, field, limit, tags } => {
            assert_eq!(query, "build");
            assert!(fuzzy);
            assert_eq!(field.as_deref(), Some("description"));
            assert_eq!(limit, Some(7));
            assert_eq!(tags, vec!["work", "python"]);
        }
        _ => panic!("expected find"),
    }

    let cli = Cli::try_parse_from([
        "aliasmgr", "--format", "json", "list", "--sort", "updated_at",
        "--desc", "--tag", "work", "--tag", "python", "--limit", "0",
    ]).unwrap();
    match cli.command {
        Command::List { sort, desc, limit, tags } => {
            assert_eq!(sort.as_deref(), Some("updated_at"));
            assert!(desc);
            assert_eq!(limit, Some(0));
            assert_eq!(tags, vec!["work", "python"]);
        }
        _ => panic!("expected list"),
    }
}
