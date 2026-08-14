mod cli;
mod commands;
mod exit_code;
mod messages;
mod output;

use clap::Parser;

fn main() {
    let cli = cli::Cli::parse();
    if let Err(error) = commands::ensure_interactive(&cli.command, atty::is(atty::Stream::Stdin)) {
        std::process::exit(exit_code::exit_code(&error));
    }
    println!("aliasmgr foundation parsed: {:?}", cli.command);
}
