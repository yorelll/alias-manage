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
    println!("aliasmgr foundation parsed: {:?}", cli.command);
}
