mod cli;
mod commands;
mod exit_code;
mod messages;
mod output;

use clap::Parser;

fn main() {
    use crate::messages::{ALIAS_ADDED, ALIAS_REMOVED, RELOAD_REQUIRED};
    use crate::output::{json, table, AliasRow};
    let _ = (ALIAS_ADDED, ALIAS_REMOVED, RELOAD_REQUIRED, table(&[]), json(&Vec::<AliasRow>::new()));
    let cli = cli::Cli::parse();
    if let Err(error) = commands::ensure_interactive(&cli.command, atty::is(atty::Stream::Stdin)) {
        std::process::exit(exit_code::exit_code(&error));
    }
    let result = match &cli.command {
        cli::Command::Add(args) => commands::add(cli.config_dir.as_deref(), args).map(|_| messages::ALIAS_ADDED.to_string()),
        cli::Command::Get { name } => commands::get(cli.config_dir.as_deref(), name).map(|value| format!("{value:?}")),
        cli::Command::List { .. } => commands::list(cli.config_dir.as_deref()).map(|values| output::table(&values.iter().map(|value| output::AliasRow { name: value.name.clone(), target: value.executable.clone(), enabled: value.enabled }).collect::<Vec<_>>())),
        cli::Command::Remove { name, .. } => commands::remove(cli.config_dir.as_deref(), name).map(|_| messages::ALIAS_REMOVED.to_string()),
        cli::Command::Enable { name } => commands::enable(cli.config_dir.as_deref(), name, true).map(|_| messages::ALIAS_ADDED.to_string()),
        cli::Command::Disable { name } => commands::enable(cli.config_dir.as_deref(), name, false).map(|_| messages::ALIAS_REMOVED.to_string()),
        _ => Ok(format!("aliasmgr foundation parsed: {:?}", cli.command)),
    };
    match result { Ok(output) => println!("{output}"), Err(error) => { eprintln!("{error}"); std::process::exit(exit_code::exit_code(&error)); } }
}
