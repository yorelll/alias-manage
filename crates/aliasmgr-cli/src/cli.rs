use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(name = "aliasmgr", version, about = "跨平台命令别名管理器")]
pub struct Cli {
    #[arg(long)] pub config_dir: Option<String>,
    #[arg(long, value_enum, default_value_t = OutputFormat::Table)] pub format: OutputFormat,
    #[arg(long)] pub no_color: bool,
    #[arg(long, conflicts_with = "quiet")] pub verbose: bool,
    #[arg(long, conflicts_with = "verbose")] pub quiet: bool,
    #[command(subcommand)] pub command: Command,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum OutputFormat { Table, Json }

#[derive(Debug, Subcommand)]
pub enum Command {
    Add(AddArgs), Remove { name: String, #[arg(long)] yes: bool }, Update(UpdateArgs), Rename { old: String, new: String }, Get { name: String },
    Find { query: String, #[arg(long)] fuzzy: bool, #[arg(long)] field: Option<String>, #[arg(long)] limit: Option<usize>, #[arg(long = "tag")] tags: Vec<String> },
    List { #[arg(long)] sort: Option<String>, #[arg(long)] desc: bool, #[arg(long)] limit: Option<usize>, #[arg(long = "tag")] tags: Vec<String> },
    Enable { name: String }, Disable { name: String }, Sync { #[arg(long)] dry_run: bool },
    Reload { #[arg(long)] print: bool, #[arg(long)] shell: Option<String> }, Doctor,
    Shell { #[command(subcommand)] command: ShellCommand },
    Import { file: String, #[arg(long)] yes: bool }, Export { file: String }, Uninstall { #[arg(long)] purge_aliases: bool, #[arg(long)] yes: bool },
}

#[derive(Debug, Args)]
pub struct AddArgs {
    pub name: String,
    #[arg(long)] pub exec: String,
    // `allow_hyphen_values` lets real-world arguments such as `--oneline` through,
    // but it also means `--arg --no-pass-args` consumes the flag as a value.
    // Use `--arg=<value>` or place other flags before `--arg` when ambiguous.
    #[arg(long = "arg", allow_hyphen_values = true)] pub args: Vec<String>,
    #[arg(long)] pub shell: Vec<String>,
    #[arg(long)] pub cwd: Option<String>,
    #[arg(long = "env")] pub environment: Vec<String>,
    #[arg(long = "tag")] pub tags: Vec<String>,
    #[arg(long)] pub no_pass_args: bool,
}

#[derive(Debug, Args)]
pub struct UpdateArgs {
    pub name: String,
    #[arg(long)] pub exec: Option<String>,
    // See the `AddArgs::args` note: `--arg --pass-args` would be consumed as a value.
    #[arg(long = "arg", allow_hyphen_values = true)] pub args: Option<Vec<String>>,
    #[arg(long)] pub shell: Option<Vec<String>>,
    #[arg(long)] pub cwd: Option<String>,
    #[arg(long = "env")] pub environment: Option<Vec<String>>,
    #[arg(long = "tag")] pub tags: Option<Vec<String>>,
    #[arg(long, conflicts_with = "pass_args")] pub no_pass_args: bool,
    #[arg(long, conflicts_with = "no_pass_args")] pub pass_args: bool,
}

#[derive(Debug, Subcommand)]
pub enum ShellCommand { Detect, Install { shell: String }, Uninstall { shell: String } }
