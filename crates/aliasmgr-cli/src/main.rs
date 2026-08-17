mod cli;
mod commands;
mod exit_code;
mod messages;
mod output;

use clap::Parser;

fn main() {
    let cli = cli::Cli::parse();
    if let Err(error) = commands::ensure_interactive(&cli.command, atty::is(atty::Stream::Stdin)) { std::process::exit(exit_code::exit_code(&error)); }
    let result: Result<String, aliasmgr_core::error::AliasError> = match &cli.command {
        cli::Command::Add(args) => commands::add(cli.config_dir.as_deref(), args).map(|_| messages::ALIAS_ADDED.to_string()),
        cli::Command::Get { name } => commands::get(cli.config_dir.as_deref(), name).map(|value| format!("{value:?}")),
        cli::Command::List { sort, desc, limit, tags } => commands::list_query(cli.config_dir.as_deref(), sort.as_deref(), *desc, *limit, tags.clone()).map(|values| output::alias_rows(&values, cli.format)),
        cli::Command::Remove { name, .. } => commands::remove(cli.config_dir.as_deref(), name).map(|_| messages::ALIAS_REMOVED.to_string()),
        cli::Command::Enable { name } => commands::enable(cli.config_dir.as_deref(), name, true).map(|_| messages::ALIAS_ADDED.to_string()),
        cli::Command::Disable { name } => commands::enable(cli.config_dir.as_deref(), name, false).map(|_| messages::ALIAS_REMOVED.to_string()),
        cli::Command::Update { name } => commands::update(cli.config_dir.as_deref(), name, "git", Vec::new()).map(|_| "别名已更新。".to_string()),
        cli::Command::Rename { old, new } => commands::rename(cli.config_dir.as_deref(), old, new).map(|_| "别名已改名。".to_string()),
        cli::Command::Find { query, fuzzy, field, limit, tags } => commands::find_query(cli.config_dir.as_deref(), query, *fuzzy, field.as_deref(), *limit, tags.clone()).map(|values| output::alias_rows(&values, cli.format)),
        cli::Command::Sync { dry_run } => commands::sync(cli.config_dir.as_deref(), *dry_run),
        cli::Command::Reload { print } => Ok(if *print { commands::reload_print(cli.config_dir.as_deref(), "bash") } else { messages::RELOAD_REQUIRED.to_string() }),
        cli::Command::Doctor => commands::doctor(cli.config_dir.as_deref()).map(|findings| findings.join("\n")),
        cli::Command::Shell { command: cli::ShellCommand::Detect } => Ok(format!("{:?}", commands::shell_detect())),
        cli::Command::Shell { command: cli::ShellCommand::Install { shell } } => Ok(format!("shell install requested: {shell}")),
        cli::Command::Shell { command: cli::ShellCommand::Uninstall { shell } } => Ok(format!("shell uninstall requested: {shell}")),
        cli::Command::Import { file } => commands::import_file(file).map(|report| format!("imported={} skipped={} unsupported={}", report.imported.len(), report.skipped.len(), report.unsupported.len())),
        cli::Command::Export { file } => commands::export_file(cli.config_dir.as_deref(), file).map(|_| format!("exported to {file}")),
        cli::Command::Uninstall { purge_aliases } => commands::uninstall(cli.config_dir.as_deref(), *purge_aliases).map(|_| "uninstall completed".into()),
    };
    match result { Ok(output) => println!("{output}"), Err(error) => { eprintln!("{error}"); std::process::exit(exit_code::exit_code(&error)); } }
}
