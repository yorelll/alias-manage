use aliasmgr_core::model::{PathMode, RestrictionMode, TargetType};
use clap::{Args, Parser, Subcommand, ValueEnum};

/// `aliasmgr` — cross-platform command alias manager.
///
/// Aliases are stored in one SQLite database under the config root. `sync`
/// renders them into self-contained shell files; a marked loader block in each
/// shell profile sources that file on startup. Run `aliasmgr help <COMMAND>`
/// for the full contract of any subcommand.
#[derive(Debug, Parser)]
#[command(
    name = "aliasmgr",
    version,
    about = "Cross-platform command alias manager",
    long_about = "Alias Manager stores command aliases in one SQLite database and renders them into \
self-contained definition files for Bash, Zsh, PowerShell 5.1, and PowerShell 7.\n\n\
The CLI, GUI, and (planned) TUI share the same config root and database. Generated files are \
derived data: `sync` rebuilds them, and a marked loader block in each shell profile sources the \
file when a new shell starts. An already-open shell must reload the file explicitly.",
    after_help = "EXAMPLES:\n  \
aliasmgr add gs --exec git --arg status --shell bash --tag git\n  \
aliasmgr list\n  \
aliasmgr sync\n  \
aliasmgr reload --print --shell bash\n  \
aliasmgr doctor\n\n\
Run `aliasmgr help <COMMAND>` or `aliasmgr <COMMAND> --help` for detailed usage and examples."
)]
pub struct Cli {
    /// Configuration root directory.
    ///
    /// Takes precedence over `ALIASMGR_CONFIG_DIR` and the platform default.
    #[arg(
        long,
        value_name = "PATH",
        help = "Configuration root directory (overrides ALIASMGR_CONFIG_DIR)."
    )]
    pub config_dir: Option<String>,
    /// Output format for `list` and `find`.
    #[arg(
        long,
        value_enum,
        default_value_t = OutputFormat::Table,
        value_name = "FORMAT",
        help = "Output format for list/find [default: table]."
    )]
    pub format: OutputFormat,
    /// Disable colored output (reserved for forward compatibility).
    #[arg(long, help = "Disable colored output (currently a reserved no-op).")]
    pub no_color: bool,
    /// Print additional diagnostics (reserved for forward compatibility).
    #[arg(
        long,
        conflicts_with = "quiet",
        help = "Print additional diagnostics (currently a reserved no-op)."
    )]
    pub verbose: bool,
    /// Suppress successful command output; errors still reach stderr.
    #[arg(
        long,
        conflicts_with = "verbose",
        help = "Suppress successful output; errors still go to stderr."
    )]
    pub quiet: bool,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum OutputFormat {
    #[value(help = "Human-readable table")]
    Table,
    #[value(help = "Machine-readable JSON")]
    Json,
}

/// CLI-exposed target types. `RawShellCommand` is deliberately absent: raw
/// shell commands are rejected by the model, and clap rejects the value as a
/// usage error (exit 2) instead of accepting an unsupported record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum TargetTypeArg {
    #[value(
        name = "native_executable",
        help = "Run the executable directly (default)"
    )]
    NativeExecutable,
    #[value(
        name = "python_script",
        help = "Classify as Python; executable is still run directly"
    )]
    PythonScript,
    #[value(
        name = "powershell_script",
        help = "Classify as PowerShell; executable is still run directly"
    )]
    PowerShellScript,
    #[value(
        name = "batch",
        help = "Run a .bat/.cmd target with extra metacharacter checks"
    )]
    Batch,
    #[value(
        name = "shell_script",
        help = "Classify as a shell script; executable is still run directly"
    )]
    ShellScript,
    #[value(
        name = "java_jar",
        help = "Classify as a Java JAR; executable is still run directly"
    )]
    JavaJar,
    #[value(
        name = "cmdlet",
        help = "Classify as a PowerShell cmdlet; executable is still run directly"
    )]
    Cmdlet,
    #[value(
        name = "change_directory",
        help = "Change the invocation directory instead of running a tool"
    )]
    ChangeDirectory,
}

impl From<TargetTypeArg> for TargetType {
    fn from(value: TargetTypeArg) -> Self {
        match value {
            TargetTypeArg::NativeExecutable => TargetType::NativeExecutable,
            TargetTypeArg::PythonScript => TargetType::PythonScript,
            TargetTypeArg::PowerShellScript => TargetType::PowerShellScript,
            TargetTypeArg::Batch => TargetType::Batch,
            TargetTypeArg::ShellScript => TargetType::ShellScript,
            TargetTypeArg::JavaJar => TargetType::JavaJar,
            TargetTypeArg::Cmdlet => TargetType::Cmdlet,
            TargetTypeArg::ChangeDirectory => TargetType::ChangeDirectory,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum PathModeArg {
    #[value(
        name = "absolute",
        help = "Treat the executable as an absolute path or PATH lookup"
    )]
    Absolute,
    #[value(
        name = "relative_to_working_directory",
        help = "Interpret the executable relative to the working directory"
    )]
    RelativeToWorkingDirectory,
}

impl From<PathModeArg> for PathMode {
    fn from(value: PathModeArg) -> Self {
        match value {
            PathModeArg::Absolute => PathMode::Absolute,
            PathModeArg::RelativeToWorkingDirectory => PathMode::RelativeToWorkingDirectory,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum RestrictionModeArg {
    #[value(
        name = "whitelist",
        help = "Allow execution only inside the listed directories"
    )]
    Whitelist,
    #[value(
        name = "blacklist",
        help = "Refuse execution inside the listed directories"
    )]
    Blacklist,
}

impl From<RestrictionModeArg> for RestrictionMode {
    fn from(value: RestrictionModeArg) -> Self {
        match value {
            RestrictionModeArg::Whitelist => RestrictionMode::Whitelist,
            RestrictionModeArg::Blacklist => RestrictionMode::Blacklist,
        }
    }
}

#[derive(Debug, Subcommand)]
pub enum Command {
    #[command(
        about = "Create a new alias",
        long_about = "Create a new alias record. The executable and its fixed arguments are stored \
separately and executed as a structured argv array; the generated shell wrapper never evaluates a \
single command string.\n\n\
User arguments are forwarded by default. Use --no-pass-args to drop them, or place {{args}} in a \
fixed argument to choose the insertion point. `{{{{args}}}}` renders the literal text {{args}}.",
        after_help = "EXAMPLES:\n  \
aliasmgr add gs --exec git --arg status --shell bash --tag git\n  \
aliasmgr add py --exec python3 --arg script.py --shell zsh --description \"Run script\"\n  \
aliasmgr add rmwork --exec rm --restrict whitelist --restrict-path /tmp/work\n\n\
Target type is classification metadata. To run `python test.py`, pass --exec python and \
--arg test.py; do not put `python test.py` in --exec."
    )]
    Add(AddArgs),
    #[command(
        about = "Delete an alias and retire its generated names",
        long_about = "Delete an alias record. A tombstone is recorded for every shell the alias \
targeted so the next sync removes the managed definition from those shells.",
        after_help = "EXAMPLES:\n  aliasmgr remove gs --yes\n  aliasmgr remove gs"
    )]
    Remove {
        /// Name of the alias to delete.
        #[arg(value_name = "NAME", help = "Alias name to delete.")]
        name: String,
        /// Skip the interactive confirmation.
        #[arg(long, help = "Skip the interactive confirmation prompt.")]
        yes: bool,
    },
    #[command(
        about = "Update fields on an existing alias",
        long_about = "Update an existing alias. Only supplied fields change; omitted fields keep \
their stored values. Supplying --arg, --shell, or --tag replaces that whole collection. --env merges \
keys into the existing environment map.\n\n\
Removing a shell from the record retires the alias name for that shell so the next sync cleans it up.",
        after_help = "EXAMPLES:\n  \
aliasmgr update gs --arg status --arg --short\n  \
aliasmgr update gs --shell bash --shell zsh\n  \
aliasmgr update gs --clear-restriction\n  \
aliasmgr update gs --restrict blacklist --restrict-path /tmp/work"
    )]
    Update(UpdateArgs),
    #[command(
        about = "Rename an alias",
        long_about = "Rename an alias and retire the old name for every bound shell. The next sync \
removes the old managed definition and writes the new one.",
        after_help = "EXAMPLES:\n  aliasmgr rename gs git-status"
    )]
    Rename {
        /// Current alias name.
        #[arg(value_name = "OLD", help = "Current alias name.")]
        old: String,
        /// New alias name.
        #[arg(value_name = "NEW", help = "New alias name.")]
        new: String,
    },
    #[command(
        about = "Print one stored alias record",
        long_about = "Print the stored record for one alias, including its shells, fixed arguments, \
environment, tags, and restriction. Output is a Rust debug representation regardless of --format.",
        after_help = "EXAMPLES:\n  aliasmgr get gs"
    )]
    Get {
        /// Alias name to print.
        #[arg(value_name = "NAME", help = "Alias name to print.")]
        name: String,
    },
    #[command(
        about = "Search aliases",
        long_about = "Search aliases by name, target, description, tags, shells, or target type. \
Without --fuzzy, exact, prefix, and substring matches still score; --fuzzy adds subsequence matching.",
        after_help = "EXAMPLES:\n  \
aliasmgr find git\n  \
aliasmgr find status --field target --limit 10\n  \
aliasmgr find work --tag git --tag daily\n  \
aliasmgr find gs --fuzzy --format json"
    )]
    Find {
        /// Free-text query.
        #[arg(value_name = "QUERY", help = "Free-text query.")]
        query: String,
        /// Enable subsequence (fuzzy) matching.
        #[arg(long, help = "Enable subsequence (fuzzy) matching.")]
        fuzzy: bool,
        /// Field to search.
        #[arg(
            long,
            value_name = "FIELD",
            help = "Field to search: all, name, target, description, tags, shells, target_type [default: all]."
        )]
        field: Option<String>,
        /// Maximum number of results.
        #[arg(
            long,
            value_name = "N",
            help = "Maximum number of results [default: 50]."
        )]
        limit: Option<usize>,
        /// Only include aliases carrying this tag (repeatable, AND).
        #[arg(
            long = "tag",
            value_name = "TAG",
            help = "Only include aliases with this tag; repeat for AND filtering."
        )]
        tags: Vec<String>,
    },
    #[command(
        about = "List stored aliases",
        long_about = "List stored aliases, optionally sorted, limited, or filtered by tag. Table \
output replaces control characters in displayed values so a crafted value cannot break columns or \
inject terminal sequences. --format json emits list-row fields, not a full record dump.",
        after_help = "EXAMPLES:\n  \
aliasmgr list\n  \
aliasmgr list --sort updated_at --desc --limit 20\n  \
aliasmgr list --tag git --format json"
    )]
    List {
        /// Sort field.
        #[arg(
            long,
            value_name = "FIELD",
            help = "Sort by name, updated_at, created_at, target_type, or enabled [default: name]."
        )]
        sort: Option<String>,
        /// Sort descending.
        #[arg(long, help = "Sort in descending order.")]
        desc: bool,
        /// Maximum number of rows.
        #[arg(long, value_name = "N", help = "Maximum number of rows [default: 50].")]
        limit: Option<usize>,
        /// Only include aliases carrying this tag (repeatable, AND).
        #[arg(
            long = "tag",
            value_name = "TAG",
            help = "Only include aliases with this tag; repeat for AND filtering."
        )]
        tags: Vec<String>,
    },
    #[command(
        about = "Enable an alias",
        long_about = "Re-enable a disabled alias. Disabled aliases stay in the database but are \
omitted from generated definition files.",
        after_help = "EXAMPLES:\n  aliasmgr enable gs"
    )]
    Enable {
        /// Alias name to enable.
        #[arg(value_name = "NAME", help = "Alias name to enable.")]
        name: String,
    },
    #[command(
        about = "Disable an alias",
        long_about = "Disable an alias without deleting it. It remains in the database and exports \
but is omitted from generated definition files until re-enabled and synced.",
        after_help = "EXAMPLES:\n  aliasmgr disable gs"
    )]
    Disable {
        /// Alias name to disable.
        #[arg(value_name = "NAME", help = "Alias name to disable.")]
        name: String,
    },
    #[command(
        about = "Generate shell definition files",
        long_about = "Generate definition files for every shell referenced by an alias or holding a \
tombstoned name. The revision is max(alias revisions, tombstone revisions, 1), so deletes never move \
the revision backwards. Each file is written under a lock, after a backup and syntax check, through a \
temporary file plus atomic rename.\n\n\
Files are written to <config-root>/generated/. An already-open shell is not updated: run the reload \
command from `aliasmgr reload --print` (or open a new shell whose profile has the loader block).",
        after_help = "EXAMPLES:\n  \
aliasmgr sync\n  \
aliasmgr sync --dry-run\n  \
aliasmgr --config-dir /tmp/aliasmgr sync\n\n\
After syncing, reload the current shell:\n  \
aliasmgr reload --print --shell bash\n  aliasmgr reload --print --shell powershell7"
    )]
    Sync {
        /// Validate inputs without writing anything.
        #[arg(
            long,
            help = "Non-mutating no-op success; does not create the database or generated files."
        )]
        dry_run: bool,
    },
    #[command(
        about = "Show how to reload generated aliases in the current shell",
        long_about = "Without --print, remind the user that a child process cannot update an \
already-open parent shell. With --print, print the shell-specific command that sources the generated \
file in the current session.\n\n\
The leading `.` in a PowerShell command is the dot-source operator (it runs the file in the current \
session); it is not punctuation. Bash and Zsh use `source`. Reloading does not modify the database. \
New shells pick up the file automatically through the loader block.",
        after_help = "EXAMPLES:\n  \
aliasmgr reload\n  \
aliasmgr reload --print --shell bash\n  \
aliasmgr reload --print --shell powershell7\n\n\
In PowerShell the leading `.` is the dot-source operator:\n  \
. 'C:\\Users\\me\\alias-manager\\generated\\powershell7.ps1'"
    )]
    Reload {
        /// Print the source/dot-source command instead of a reminder.
        #[arg(
            long,
            help = "Print the source/dot-source command instead of a reminder."
        )]
        print: bool,
        /// Shell whose reload command should be printed.
        #[arg(
            long,
            value_name = "SHELL",
            help = "Target shell: bash, zsh, powershell5, powershell7 [default: detected, else bash]."
        )]
        shell: Option<String>,
    },
    #[command(
        about = "Inspect configuration, generated files, and loaders",
        long_about = "Report missing database, missing generated files, stale generated files, sync \
errors, missing loader blocks, checksum mismatches, and filesystem-reliability warnings. Doctor is \
read-only: it does not repair anything. Use `sync` and `shell install` to act on findings.",
        after_help = "EXAMPLES:\n  aliasmgr doctor"
    )]
    Doctor,
    #[command(
        about = "Detect or manage shell integration",
        long_about = "Inspect the detected shell and install or remove the marked Alias Manager loader \
block in a shell profile. Installation edits only the marked block, creates a backup, and honors the \
configured RC retention limit.",
        after_help = "EXAMPLES:\n  \
aliasmgr shell detect\n  \
aliasmgr shell install powershell7\n  \
aliasmgr shell uninstall bash"
    )]
    Shell {
        #[command(subcommand)]
        command: ShellCommand,
    },
    #[command(
        about = "Import aliases from JSON or TOML",
        long_about = "Read a JSON or TOML export, validate every candidate with the same rules as \
`add`, and preview or persist the accepted records. Without --yes the command only previews: it opens \
the database read-only and writes nothing. With --yes accepted records are persisted atomically. \
Invalid records are skipped with warnings and are never persisted.",
        after_help = "EXAMPLES:\n  \
aliasmgr import aliases.json\n  \
aliasmgr import aliases.toml --yes"
    )]
    Import {
        /// Import file (.json or .toml).
        #[arg(value_name = "FILE", help = "Import file (.json or .toml).")]
        file: String,
        /// Persist the accepted records.
        #[arg(long, help = "Persist accepted records instead of only previewing.")]
        yes: bool,
    },
    #[command(
        about = "Export aliases to JSON or TOML",
        long_about = "Write every alias to a JSON (default) or TOML file. Sensitive environment keys \
(PASSWORD, TOKEN, SECRET, API_KEY, APIKEY) are omitted from the export. The same envelope is \
importable on every supported shell.",
        after_help = "EXAMPLES:\n  aliasmgr export aliases.json\n  aliasmgr export aliases.toml"
    )]
    Export {
        /// Export file; a .toml extension selects TOML.
        #[arg(
            value_name = "FILE",
            help = "Export file; the .toml extension selects TOML, anything else JSON."
        )]
        file: String,
    },
    #[command(
        about = "Remove Alias Manager's loader blocks and optionally its data",
        long_about = "Remove marked loader blocks from configured shell profiles first, then either \
retain the database and generated files (default) or purge them with --purge-aliases. Referenced \
target files are never deleted. Purge requires --yes in a non-interactive process.",
        after_help = "EXAMPLES:\n  \
aliasmgr uninstall\n  \
aliasmgr uninstall --purge-aliases --yes"
    )]
    Uninstall {
        /// Also delete the database and generated files.
        #[arg(
            long,
            help = "Also delete the database and generated definition files."
        )]
        purge_aliases: bool,
        /// Skip the interactive confirmation for purge.
        #[arg(long, help = "Skip the interactive confirmation prompt.")]
        yes: bool,
    },
}

#[derive(Debug, Args)]
pub struct AddArgs {
    /// Alias name.
    #[arg(
        value_name = "NAME",
        help = "Alias name: start with a letter or underscore; letters, digits, _, - only; max 64."
    )]
    pub name: String,
    /// Program or command to execute.
    #[arg(
        long,
        value_name = "PROGRAM",
        help = "Program or command to execute. Do not include arguments here."
    )]
    pub exec: String,
    // `allow_hyphen_values` lets real-world arguments such as `--oneline` through,
    // but it also means `--arg --no-pass-args` consumes the flag as a value.
    // Use `--arg=<value>` or place other flags before `--arg` when ambiguous.
    #[arg(
        long = "arg",
        value_name = "VALUE",
        allow_hyphen_values = true,
        help = "Fixed argument passed before user arguments; repeatable; accepts leading hyphens."
    )]
    pub args: Vec<String>,
    /// Shells this alias targets.
    #[arg(
        long,
        value_name = "SHELL",
        help = "Target shell: bash, zsh, powershell5, powershell7; repeatable [default: bash]."
    )]
    pub shell: Vec<String>,
    /// Working directory for the generated invocation.
    #[arg(
        long,
        value_name = "PATH",
        help = "Run the target in this working directory without changing the caller's shell."
    )]
    pub cwd: Option<String>,
    /// Environment variable for the generated invocation.
    #[arg(
        long = "env",
        value_name = "KEY=VALUE",
        help = "Set an environment variable for the generated invocation; repeatable."
    )]
    pub environment: Vec<String>,
    /// Tags for filtering and search.
    #[arg(long = "tag", value_name = "TAG", help = "Attach a tag; repeatable.")]
    pub tags: Vec<String>,
    /// Free-form description or memo.
    #[arg(long, value_name = "TEXT", help = "Free-form description or memo.")]
    pub description: Option<String>,
    /// Execution-directory restriction mode.
    #[arg(
        long,
        value_enum,
        value_name = "MODE",
        help = "Restrict execution by directory: whitelist or blacklist; requires --restrict-path."
    )]
    pub restrict: Option<RestrictionModeArg>,
    /// Directory for the execution restriction.
    #[arg(
        long = "restrict-path",
        value_name = "PATH",
        help = "Absolute directory for the restriction; repeatable; requires --restrict. See docs/restrictions.md."
    )]
    pub restrict_paths: Vec<String>,
    /// Do not forward user arguments.
    #[arg(
        long,
        help = "Do not forward user arguments to the target [default: forward]."
    )]
    pub no_pass_args: bool,
    /// Classification metadata for the target.
    #[arg(
        long,
        value_enum,
        value_name = "TYPE",
        help = "Target type metadata: native_executable, python_script, powershell_script, batch, shell_script, java_jar, cmdlet, change_directory."
    )]
    pub target_type: Option<TargetTypeArg>,
    /// How to interpret the executable path.
    #[arg(
        long,
        value_enum,
        value_name = "MODE",
        help = "Path mode: absolute (default) or relative_to_working_directory."
    )]
    pub path_mode: Option<PathModeArg>,
}

#[derive(Debug, Args)]
pub struct UpdateArgs {
    /// Alias name to update.
    #[arg(value_name = "NAME", help = "Alias name to update.")]
    pub name: String,
    /// Replace the executable.
    #[arg(
        long,
        value_name = "PROGRAM",
        help = "Replace the executable. Omit to keep the stored value."
    )]
    pub exec: Option<String>,
    // See the `AddArgs::args` note: `--arg --pass-args` would be consumed as a value.
    #[arg(
        long = "arg",
        value_name = "VALUE",
        allow_hyphen_values = true,
        help = "Replace the whole fixed-argument list; repeatable; accepts leading hyphens."
    )]
    pub args: Option<Vec<String>>,
    /// Replace the targeted shells.
    #[arg(
        long,
        value_name = "SHELL",
        help = "Replace the shell list: bash, zsh, powershell5, powershell7; repeatable."
    )]
    pub shell: Option<Vec<String>>,
    /// Replace the working directory.
    #[arg(long, value_name = "PATH", help = "Replace the working directory.")]
    pub cwd: Option<String>,
    /// Merge environment variables.
    #[arg(
        long = "env",
        value_name = "KEY=VALUE",
        help = "Merge environment variables into the stored map; repeatable."
    )]
    pub environment: Option<Vec<String>>,
    /// Replace the tag list.
    #[arg(
        long = "tag",
        value_name = "TAG",
        help = "Replace the whole tag list; repeatable."
    )]
    pub tags: Option<Vec<String>>,
    /// Replace the description.
    #[arg(
        long,
        value_name = "TEXT",
        help = "Replace the description; an empty string clears it."
    )]
    pub description: Option<String>,
    /// Change the restriction mode.
    #[arg(
        long,
        value_enum,
        value_name = "MODE",
        help = "Change the restriction mode, keeping existing paths unless --restrict-path is also given."
    )]
    pub restrict: Option<RestrictionModeArg>,
    /// Replace the restriction paths.
    #[arg(
        long = "restrict-path",
        value_name = "PATH",
        help = "Replace the restriction path list; repeatable; requires an existing restriction or --restrict."
    )]
    pub restrict_paths: Vec<String>,
    /// Remove the execution restriction.
    #[arg(
        long,
        conflicts_with_all = ["restrict", "restrict_paths"],
        help = "Remove the execution restriction entirely."
    )]
    pub clear_restriction: bool,
    /// Do not forward user arguments.
    #[arg(
        long,
        conflicts_with = "pass_args",
        help = "Stop forwarding user arguments."
    )]
    pub no_pass_args: bool,
    /// Forward user arguments.
    #[arg(
        long,
        conflicts_with = "no_pass_args",
        help = "Start forwarding user arguments."
    )]
    pub pass_args: bool,
    /// Replace the target-type classification.
    #[arg(
        long,
        value_enum,
        value_name = "TYPE",
        help = "Replace the target type; same values as add."
    )]
    pub target_type: Option<TargetTypeArg>,
    /// Replace the path mode.
    #[arg(
        long,
        value_enum,
        value_name = "MODE",
        help = "Replace the path mode: absolute or relative_to_working_directory."
    )]
    pub path_mode: Option<PathModeArg>,
}

#[derive(Debug, Subcommand)]
pub enum ShellCommand {
    #[command(
        about = "Report the detected shell",
        long_about = "Report which shell Alias Manager detects for the current process and why \
(explicit setting, parent process, SHELL environment variable, login shell, or installed fallback).",
        after_help = "EXAMPLES:\n  aliasmgr shell detect"
    )]
    Detect,
    #[command(
        about = "Install the loader block in a shell profile",
        long_about = "Install the marked Alias Manager loader block in the profile for one shell. The \
block sources the generated file only when it exists, so a missing file cannot break profile startup. \
A configured profile path overrides the platform default; profile writes create a backup and honor \
the RC retention limit.",
        after_help = "EXAMPLES:\n  \
aliasmgr shell install bash\n  \
aliasmgr shell install powershell7\n\n\
PowerShell 5.1 does not exist outside Windows and returns exit 12 there."
    )]
    Install {
        /// Shell whose loader block should be installed.
        #[arg(
            value_name = "SHELL",
            help = "Target shell: bash, zsh, powershell5, powershell7."
        )]
        shell: String,
    },
    #[command(
        about = "Remove the loader block from a shell profile",
        long_about = "Remove the marked Alias Manager loader block from one shell profile. Only the \
marked block is removed; unrelated profile content is preserved. A missing block is a successful \
no-op.",
        after_help = "EXAMPLES:\n  aliasmgr shell uninstall bash"
    )]
    Uninstall {
        /// Shell whose loader block should be removed.
        #[arg(
            value_name = "SHELL",
            help = "Target shell: bash, zsh, powershell5, powershell7."
        )]
        shell: String,
    },
}
