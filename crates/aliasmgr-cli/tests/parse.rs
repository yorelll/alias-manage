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
