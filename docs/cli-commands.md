# CLI Commands

This document describes the current CLI surface (the TUI is launched from `aliasmgr tui`). It is a command contract verified against `crates/aliasmgr-cli` and the core library on 2026-11-04, not a claim that every environment-dependent Shell behavior is accepted on every machine. CI and tests always use an isolated temporary configuration directory.

## Global options

These are root-level clap options, not per-subcommand options. They must appear before the subcommand: `aliasmgr --format json list` works, while `aliasmgr list --format json` is rejected as a usage error (exit 2).

- `--config-dir <PATH>`: explicit configuration root; takes precedence over `ALIASMGR_CONFIG_DIR`.
- `--format table|json`: output selection for `list` and `find`. `get` always prints the record in Rust debug form, regardless of `--format`.
- `--quiet`: suppresses successful command stdout. Errors are still written to stderr and keep their exit code.
- `--no-color`, `--verbose`: accepted for forward compatibility but currently reserved no-ops. They do not change output.

See `docs/exit-codes.md` for the stable error mapping.

## Configuration root precedence

Every command that touches configuration resolves one root through `AppPaths::discover`:

1. `--config-dir <PATH>` when supplied.
2. `ALIASMGR_CONFIG_DIR` when set to a non-empty value. An empty value is treated as unset.
3. Platform default:
   - Windows: `%LOCALAPPDATA%\alias-manager`, then `%APPDATA%\alias-manager`, then `.alias-manager` in the current directory.
   - Linux: `$XDG_CONFIG_HOME/alias-manager`, then `$HOME/.config/alias-manager`, then `.alias-manager` in the current directory.

The database, generated files, backups, journal, loader paths, `sync`, `reload --print`, and `uninstall` all use this same root. There is no separate current-directory database fallback.

## Shell integration and save-is-done

The user's intent is stored in `config.toml`; loader validity is detected on every run and is never persisted as a fact:

```toml
[runtime]
auto_sync = true

[shells]
enabled = ["powershell5", "powershell7"]   # stable ShellKind::as_str() spellings
bash_rc_path = "..."                       # existing profile overrides are unchanged
powershell7_profile_path = "..."

[shells.executables]                       # optional non-standard install paths
powershell7 = "C:\\Program Files\\PowerShell\\7\\pwsh.exe"
bash = "C:\\Program Files\\Git\\bin\\bash.exe"

[shells.seen]                              # GUI prompt dismissal memory
bash = "C:\\Program Files\\Git\\bin\\bash.exe"
```

- `enabled` is the user's intent. It defaults to `[]`; loading a configuration written before this key existed does **not** enable anything. Only an explicit `doctor`/settings action writes it. Use `bash`, `zsh`, `powershell5`, and `powershell7`; serde spellings such as `power_shell5` and unsupported shells (`fish`, `posix_sh`) are rejected on save.
  - **Upgrade note:** an existing installation therefore has an empty `enabled` list, and `add`/`update` return `ShellNotReady` (exit 12) until `aliasmgr doctor` runs once and the user confirms the shells. This is deliberate: nothing is auto-enabled.
- `shells.executables.<shell>` is the highest-priority detection source. A configured but missing path is reported as not detected; it does not silently fall back to `PATH`. Override paths must be absolute; a leading `~` is expanded from `HOME`/`USERPROFILE` on load and in interactive prompts.
- `shells.seen.<shell>` is GUI-only prompt memory: the executable path the user dismissed or disabled. It never enables or detects anything; a changed executable path prompts once again. The CLI does not read or write it, and `enabled` remains the only user intent that controls generation.
- `runtime.auto_sync` defaults to `true` for old and new configurations. Set it to `false` to require an explicit `aliasmgr sync`.
- There is **no `setup` command**. First-run integration happens in `aliasmgr doctor` for the CLI and in the GUI's first-run panel (or Settings → Shell integration) for the desktop app. `doctor` lists all four supported shells even when detection fails, and requires confirmation before writing `enabled`, a profile path, a loader block, or syncing.
- Detection is read-only and best effort. It checks `PATH`, known Windows install paths (PowerShell 5.1 system path, PowerShell 7 `Program Files`/`WindowsApps`, Git Bash, MSYS2/Cygwin Zsh) and Unix `/bin`, `/usr/bin`, `/usr/local/bin`. Version probes are bounded at about two seconds; a failed or timed-out probe leaves the version unknown but does not mark the shell undetected. A Windows `System32\bash.exe`/WindowsApps `bash.exe` is reported as WSL (`wsl=true`); Alias Manager does not write a Windows profile for it — run `doctor` inside the distribution instead.
- `add` and `update` preflight every **target** shell before writing. A target that is not in `shells.enabled`, or whose loader block is not installed/verifiable, is refused with `ShellNotReady` (exit 12) and a pointer to `aliasmgr doctor`. Non-target shells are not checked.
  - `--allow-missing-loader` saves anyway with a warning on stderr. The alias is in the database but will not take effect in that shell until integration is repaired.
  - `--no-sync` skips the post-save sync and also skips the loader check (the enabled check still runs).
- After a successful `add`, `update`, `remove`, `rename`, `enable`, or `disable` write, the affected shells are synced once when `runtime.auto_sync` is true and `--no-sync` is absent. Affected shells are the record's old ∪ new shells for `add`/`update`, and the record's shells for `remove`/`rename`/`enable`/`disable`. Sync targets are independent of `shells.enabled`; the renderer output does not depend on that list, and only enabled shells have loaders.
  - A partial sync failure prints the per-shell errors and exits with `PartialSync` (exit 10). The database write is already committed and is not rolled back; run `aliasmgr sync` or `aliasmgr doctor` to retry.
  - `--no-sync`, `runtime.auto_sync = false`, or no supported affected shell returns the normal success exit code (0).
  - Import preview never syncs. A confirmed `import --yes` with accepted records syncs all referenced shells once (honoring `runtime.auto_sync` and `import --no-sync`); a sync failure after the atomic insert is reported as `PartialSync` (exit 10).

## TUI

- `tui`: launch the interactive terminal UI (see [docs/tui.md](tui.md)).
  - Requires an interactive stdin and stdout. In a pipe or CI session it exits
    with a `Config` error (exit 1) and does not enter raw mode.
  - The `tui` feature is on by default. A `--no-default-features` build rejects
    the command with a clear "TUI support is not compiled in" message (exit 1).
  - The TUI reads/writes the same `<config-root>/aliases.db`, runs the same
    sync/doctor/loader core helpers, uses the same `transfer` import/export
    code and `AppConfig`/`config.toml` semantics, and never reloads the current
    shell. It
    does not change the output, arguments or exit codes of any other command.
  - `NO_COLOR` (presence), `TERM=dumb` and `ALIASMGR_TUI_ASCII=1` control
    colour and ASCII fallback inside the TUI; the CLI's reserved `--no-color`
    flag is unchanged.

## Alias lifecycle

- `add <NAME> --exec <PROGRAM> [--arg VALUE]... [--shell SHELL]... [--cwd PATH] [--env KEY=VALUE]... [--tag TAG]... [--description TEXT] [--restrict MODE] [--restrict-path PATH]... [--no-pass-args] [--target-type TYPE] [--path-mode MODE] [--no-sync] [--allow-missing-loader]`
  - `--arg` is repeatable and accepts leading hyphens, so `--arg --oneline` is valid. Because hyphen values are allowed, a following flag can be consumed as a value when it is ambiguous; use `--arg=--oneline` or place other flags before `--arg` in that case.
  - `--shell` accepts `bash`, `zsh`, `powershell5`, and `powershell7`, and is repeatable. When omitted, the alias targets Bash only.
  - `--env KEY=VALUE` is repeatable and is stored on the record. A missing `=`, an empty key, or an invalid shell is a configuration error (exit 1).
  - `--cwd PATH` applies to each generated invocation without changing the caller's shell: Bash/Zsh run the command in a subshell after `cd`; PowerShell uses `Push-Location` and restores it in `finally`.
  - `--env` values apply only to the generated invocation: Bash/Zsh invoke the target through `env KEY=VALUE ...`; PowerShell saves, sets, and restores each variable in `finally`.
  - `--no-pass-args` sets `pass_args=false`; user arguments are forwarded by default. When fixed arguments contain `{{args}}`, user arguments are inserted at that position; `{{{{args}}}}` renders the literal text `{{args}}`. Without a placeholder, user arguments are appended at the end only when `pass_args` is true.
  - `--target-type TYPE` accepts `native_executable` (default), `python_script`, `powershell_script`, `batch`, `shell_script`, `java_jar`, `cmdlet`, and `change_directory`. `raw_shell_command` is not exposed as a CLI value and is rejected by clap as a usage error (exit 2); advanced Shell mode remains rejected by validation. See [Target type semantics](#target-type-semantics).
  - `--path-mode MODE` accepts `absolute` (default) and `relative_to_working_directory`.
  - `--description TEXT` stores a free-form memo. `--description ""` stores an empty description.
  - `--restrict whitelist|blacklist` with one or more `--restrict-path PATH` attaches an execution-directory restriction. `--restrict` without a path and `--restrict-path` without `--restrict` are configuration errors (exit 1); path validation failures are validation errors (exit 5). `--clear-restriction` is an `update`-only flag; `add` rejects it as a usage error (exit 2). Paths are trimmed, deduplicated, must be absolute on the current platform, at most 64 paths of at most 4096 characters each. See `docs/restrictions.md`.
- `get <NAME>`: prints the stored record. A missing alias is `NotFound` (exit 4).
- `list [--sort FIELD] [--desc] [--limit N] [--tag TAG]...`
  - `--sort` accepts `name` (default), `updated_at`, `created_at`, `target_type`, and `enabled`.
  - `--limit` defaults to 50. Repeated `--tag` values use AND intersection.
  - Table output columns are `NAME`, `TARGET`, `DESCRIPTION`, `TAGS`, `ENABLED`. Control characters (tabs, newlines, escape sequences) in displayed values are replaced with spaces so a crafted value cannot break columns or inject terminal sequences. `--format json` emits the list-row fields, including `description`, `tags`, and `restriction`; it is not a full record dump (it omits `id`, `path_mode`, `path_origin`, `advanced_shell_mode`, timestamps, and `record_checksum`).
- `find <QUERY> [--fuzzy] [--field FIELD] [--limit N] [--tag TAG]...`
  - `--field` accepts `all` (default), `name`, `target`, `description`, `tags`, `shells`, and `target_type`.
  - `--fuzzy` adds subsequence matching on top of exact/prefix/substring scoring. Without it, exact, prefix, and substring matches still score; there is no separate exact-only mode.
- `update <NAME> [--exec <PROGRAM>] [--arg VALUE]... [--shell SHELL]... [--cwd PATH] [--env KEY=VALUE]... [--tag TAG]... [--description TEXT] [--restrict MODE] [--restrict-path PATH]... [--clear-restriction] [--no-pass-args|--pass-args] [--target-type TYPE] [--path-mode MODE] [--no-sync] [--allow-missing-loader]`
  - Only supplied fields change. Omitted fields keep their stored values.
  - `--arg`, `--shell`, and `--tag` replace their whole collection when supplied. `--env` merges keys into the existing environment map. `--cwd` replaces the stored working directory; there is currently no flag to clear it.
  - `--description` replaces the stored memo; `--description ""` clears it.
  - Restriction updates: `--restrict` alone changes only the mode and keeps existing paths (error if the record has no restriction and no paths are supplied); `--restrict-path` alone replaces the path list and keeps the existing mode (error if the record has no restriction); supplying both replaces both; `--clear-restriction` removes the restriction. Omitted restriction flags preserve the stored value. Validation and path rules match `add`. `--clear-restriction` conflicts with `--restrict` and `--restrict-path` (clap usage error, exit 2). See `docs/restrictions.md`.
  - Removing a shell from the record retires the old name for that shell so the next `sync` cleans it up.
  - `--target-type` and `--path-mode` accept the same values as `add` and replace the stored values when supplied; omitted values are preserved.
  - A missing alias is `NotFound` (exit 4). An invalid shell or environment entry is a configuration error (exit 1). `--no-pass-args` and `--pass-args` conflict and are rejected by clap (exit 2). An unsupported `--target-type` or `--path-mode` value is a clap usage error (exit 2). Target-shell preflight and `--allow-missing-loader` are described in [Shell integration and save-is-done](#shell-integration-and-save-is-done).
- `rename <OLD> <NEW> [--no-sync]`: retires the old name for every bound shell and updates the record. A missing source is `NotFound` (exit 4); a name conflict is exit 3.
- `enable <NAME> [--no-sync]` / `disable <NAME> [--no-sync]`: flips the `enabled` flag. A missing alias is `NotFound` (exit 4).
- `remove <NAME> [--yes] [--no-sync]`: retires the name for every bound shell (tombstones advance the revision) and deletes the row. A missing alias is `NotFound` (exit 4). Without `--yes` in a non-interactive process, the command fails with `ConfirmationRequired` (exit 14).

## Target type semantics

Target type is **classification and validation metadata**, not an interpreter
launcher. Only `batch` and `change_directory` change runtime behavior. Every
other type runs the `executable` field directly through the structured executor.

| Type | Runtime behavior | Example |
| --- | --- | --- |
| `native_executable` (default) | Runs `executable` + fixed args as an argv array. | `--exec git --arg status` |
| `python_script` | Same structured execution; it does **not** prepend `python`. | `--exec python3 --arg script.py` |
| `powershell_script` | Same structured execution; pick the PowerShell host yourself. | `--exec pwsh --arg -File --arg build.ps1` |
| `shell_script` | Same structured execution; pick `bash`/`sh`/`zsh` yourself. | `--exec bash --arg deploy.sh` |
| `java_jar` | Same structured execution; pass `java -jar` explicitly. | `--exec java --arg -jar --arg app.jar` |
| `cmdlet` | Same structured execution; PowerShell invokes the named command. | `--exec Get-ChildItem` |
| `batch` | Structured execution plus extra metacharacter validation (`% ! & \| ^ < >`). | `--exec tool.bat --arg safe` |
| `change_directory` | The generated wrapper changes to the target directory instead of running a program. | `--exec /srv/project` |

To run `python test.py`, set `executable` to `python` (or `python3`) and put
`test.py` in fixed arguments. Do **not** put `python test.py` in the executable
field: a `native_executable` record with that value tries to run a program
literally named `python test.py` and fails. The GUI wizard shows this guidance
next to the target-type selector and displays an advisory warning (never a
silent rewrite) when the executable contains whitespace.

## Synchronization and diagnostics

- `sync [--dry-run]`
  - Without `--dry-run`, generates every Shell that is referenced by an alias or has a retired name.
  - The revision is `max(alias revisions, tombstone revisions, 1)`, so deletes cannot move the revision backwards.
  - Each generated file is written under a lock, after backing up the previous file, after a syntax check, and through a temporary file plus atomic rename. The file carries a revision header and a checksum line. Checksums ignore the UTF-8 BOM and the checksum line itself.
  - A syntax-check failure preserves the previous generated file and records a failed `shell_state` with the error. If any Shell fails, `sync` returns `PartialSync` (exit 10). The core-level `SyntaxCheck` error remains exit 9 but is surfaced by `sync` as part of exit 10.
  - PowerShell 5.1 files are UTF-8 with BOM by default (`runtime.powershell5_bom=true`); PowerShell 7 files are UTF-8 without BOM by default (`runtime.powershell7_bom=false`). Both are configurable in `config.toml`.
  - PowerShell line endings are configurable with `runtime.powershell5_line_ending` and `runtime.powershell7_line_ending`, set to `"crlf"` or `"lf"`. The default is LF. Checksums normalize CRLF to LF, so a generated file verifies regardless of the configured line ending.
  - `--dry-run` is a non-mutating no-op success. It does not create the database, generated files, or backups.
- `reload [--print] [--shell <SHELL>]`
  - Without `--print`, prints the reminder that an already-open parent Shell is not updated by a child process.
  - With `--print`, prints the Shell-specific command that sources the generated file. `--shell` accepts `bash`, `zsh`, `powershell5`, and `powershell7`; when omitted, the Shell is detected and falls back to Bash. An unknown explicit Shell is a configuration error (exit 1).
  - `reload` never modifies files or Shell state by itself.

### What "reload" means

Reloading makes the **current** shell re-read the generated definition file. It
is needed after aliases change, or after a sync in an already-open shell. New
shells and terminals pick the file up automatically through the loader block in
the shell profile. Reloading does not modify the database.

The printed command depends on the shell:

- Bash/Zsh: `source '<config-root>/generated/bash.sh'` (the POSIX `.` operator is
  equivalent to `source`).
- PowerShell 5.1/7: `. '<config-root>\generated\powershell7.ps1'`. The leading
  `.` is PowerShell's **dot-source operator** — it runs the file in the current
  session. It is not a comma, and it is not punctuation.

Paths are single-quoted for the target shell, so spaces and non-ASCII directory
names survive. PowerShell doubles embedded single quotes; POSIX shells use
`'\''`.

- `doctor [--format table|json]`: reports missing database, generated-file and checksum problems, staleness, recorded sync errors, loader problems, and a filesystem-reliability finding when one is available.
  - When stdin **and** stdout are TTYs and no `--format` is supplied, `doctor` opens the interactive Shell integration screen: it lists all four supported shells with detection path/version/loader state, uses Space to select, Enter to preview and apply, `r` to rescan, and `q` to quit. It pre-checks shells that are already enabled or have a valid loader, but nothing is written until the user confirms. Detected shells are previewed with the exact profile target and marked block before the loader install and sync. Not-detected shells offer install-then-restart instructions, manual executable/profile entry with defaults, or cancel; WSL bash is shown as "integrate inside the distribution". Version probes run in the background and never show a fake percentage.
  - Non-TTY sessions (pipes, CI, redirected output) keep the text report and never enter raw mode or wait for input. `--format json` always forces a machine-readable JSON array of findings and disables the interactive screen, even on a TTY; it never prompts. `--format table` explicitly selects the text report.
  - Doctor is read-only with respect to the database/generated files. The interactive screen writes only after confirmation: `shells.enabled`, optional `shells.executables`/profile overrides, the marked loader block, and the affected generated file.
  - The text report and JSON findings are unchanged from earlier releases; the findings below are additive. Doctor's exit code stays 0 for a completed report and uses the normal `AliasError` mapping for I/O or configuration failures.
  - Doctor inspects database presence, generated files and checksums, staleness, sync errors, loader installation, and filesystem reliability. It is read-only and shares its implementation between CLI and GUI.
  - Generated-file checks cover every shell referenced by an alias or tombstone, plus any leftover generated file that still exists (leftover files are validated but unreferenced shells are not reported as missing). It recomputes each file's embedded `# file_checksum:` and reports `生成文件校验和不匹配（可能被手工修改）` for a hand-edited file or a missing checksum line.
  - Target shells and the expected revision come from the same core helpers as `sync`, so a revision-0 record cannot create a permanent unfixable stale finding.
  - Loader checks probe the actual profile at the resolved path (configured path or platform fallback) and compare the managed block against the expected loader for the current config root. Findings use stable codes:
    - `loader_missing`: no expected block (or a mismatched/relocated block).
    - `loader_shared_path`: multiple shells resolve to one profile path, but the installed block sources only a subset of their generated files. This finding has no install button; give each shell a distinct profile path.
    - `loader_permission` / `loader_io` / `loader_unreadable`: the profile could not be read (permission, filesystem, or encoding).
    - `loader_unsafe`: the profile has missing, duplicated, inverted, or otherwise unpaired Alias Manager markers.
    - `config_invalid`: `config.toml` cannot be parsed **or a shell profile override is empty/non-absolute**, so loader paths cannot be resolved. Doctor still completes its report and exits 0 for this finding; `shell install`/`shell uninstall` and other commands that need a loader target fail with the normal `Config` error (exit 1).
  - Other codes: `database_missing`, `generated_missing`, `generated_unreadable`, `generated_modified`, `render_failed`, `stale`, `sync_failed`, `unreliable_filesystem`.
  - Loader and generated-file details (paths, checksum mismatches, parse errors) are reported as diagnostic detail and never auto-repaired.
  - The GUI Doctor page adds an action button to actionable findings: **Sync now** for `generated_missing`, `stale`, `sync_failed`, `generated_unreadable`, `generated_modified`, and `render_failed`; **Install shell integration** for `loader_missing`. `loader_shared_path`, marker, permission, I/O, encoding, and config findings have no install button. Actions run the same core operations as `sync` and `shell install`, then rescan automatically. The non-interactive CLI has no action buttons: run `aliasmgr sync` or `aliasmgr shell install <SHELL>` yourself, or use interactive `aliasmgr doctor`.
- `shell detect`: reports the detected Shell.
- `shell install <SHELL>` / `shell uninstall <SHELL>`: installs or removes the marked loader block for `bash`, `zsh`, `powershell5`, or `powershell7`. Other Shell names return `ShellNotInstalled` (exit 12).
  - The loader path comes from `config.toml` (`shells.bash_rc_path`, `shells.zsh_rc_path`, `shells.powershell5_profile_path`, `shells.powershell7_profile_path`). Override values must be absolute; a leading `~` is expanded from the user home on load and in interactive path prompts. Relative overrides are configuration errors: `shell install`/`shell uninstall` exit 1 without touching a cwd-relative file, while non-interactive `doctor` reports `config_invalid` and still exits 0.
  - Bash/Zsh default to `$HOME/.bashrc` / `$HOME/.zshrc` (on Windows `%USERPROFILE%\.bashrc` / `.zshrc`). This is the real shell home, so a first-run integration is loaded by a new shell. Older releases wrote `<config-root>/.bashrc` / `.zshrc`; `uninstall` still visits that fallback to clean those blocks, but install/status no longer use it.
  - On Windows, PowerShell 5.1 defaults to `%USERPROFILE%\Documents\WindowsPowerShell\Microsoft.PowerShell_profile.ps1` and PowerShell 7 to `%USERPROFILE%\Documents\PowerShell\Microsoft.PowerShell_profile.ps1`.
  - Outside Windows, PowerShell 7 defaults to `~/.config/powershell/profile.ps1`; PowerShell 5.1 without a configured path returns `ShellNotInstalled`, because that Shell does not exist there.
  - Default paths are derived from `%USERPROFILE%` / `~`; OneDrive-redirected Documents folders are not detected through the Windows known-folder API, so set the corresponding `shells.powershell*_profile_path` explicitly in that case.
  - The PowerShell block dot-sources the generated `.ps1` only when it exists and then removes its temporary variable; a missing generated file cannot break profile startup. Profile writes create a backup and respect the configured RC retention limit.
  - `shell install` is idempotent only when the existing block points at the *current* config root and sits at the end of the profile. A block pointing at another `ALIASMGR_CONFIG_DIR`, a relocated block, or a hand-edited block is replaced with the expected one.
  - `uninstall` uses the shared `integration` loader resolution: every configured path plus its built-in fallback candidate (config-root RC for Bash/Zsh, platform-default profile for PowerShell) is visited, so a loader installed without an explicit `shells.*_profile_path` is still removed.

## Transfer

- `export <FILE>`: writes JSON by default, or TOML when the path ends in `.toml`. Environment keys whose names contain `PASSWORD`, `TOKEN`, `SECRET`, `API_KEY`, or `APIKEY` (case-insensitive) are omitted from the export. The same envelope is the single import/export format for Bash, Zsh, PowerShell 5.1, and PowerShell 7: each record carries its `shells` list, plus `description`, `tags`, and an optional `restriction`. Importing into an empty database preserves those fields.
- `import <FILE> [--yes] [--no-sync]`
  - The import file is parsed and every candidate record is validated with the same rules as `add` (alias-name syntax, PowerShell-reserved names, argument templates, restriction paths). Restriction paths are trimmed, blank entries dropped, and duplicates removed before validation and persistence. Records that fail validation are skipped with a warning and are never persisted; a record whose restriction path is absolute only on another platform is skipped wholesale rather than imported without its restriction. Sensitive environment keys are filtered on import as well.
  - Without `--yes`, the command previews only: it opens the existing database read-only, evaluates conflicts through `ConflictStrategy::Ask`, and reports `imported`, `skipped`, `unsupported`, and `warnings` counts without persisting anything. Each warning is printed to stderr (for example an omitted sensitive-key name or a rejected invalid record); warning text never includes environment values, and echoed names/keys have control characters stripped and are truncated for display. The read-only SQLite handle may leave `-wal` / `-shm` side files next to the database.
  - With `--yes`, accepted records are persisted atomically in one transaction. Existing-name conflicts are reported as skipped without a warning; records rejected by validation are reported as skipped with a warning. Neither is persisted. Unsupported records are counted and are never persisted.
  - After a confirmed import with at least one accepted record, all referenced shells are synced once when `runtime.auto_sync` is true. `--no-sync` skips that sync. Preview never syncs. A sync failure occurs after the database commit and returns `PartialSync` (exit 10) with a message that the imported records were saved.
  - Preview cannot model every insert-time conflict. For example, PowerShell case-fold persistence can reject or rename a record when it is actually written.

## Uninstall

`uninstall [--purge-aliases] [--yes]`

1. Loader cleanup runs first. For every shell it visits every candidate profile: the configured path (when set), the platform-default home RC (`$HOME/.bashrc` / `.zshrc`, or `%USERPROFILE%` on Windows), and the older `<config-root>/.bashrc` / `.zshrc` fallback for Bash/Zsh; PowerShell uses its platform-default profile. This removes blocks written before an override was configured. A profile is rewritten only when a block was removed, with a backup and the configured RC retention limit. A malformed, unpaired, duplicated, or out-of-order marker aborts before any destructive step.
2. Retain mode (default) keeps `generated/` and `aliases.db`. Generated files are self-contained and do not require the `aliasmgr` program at Shell startup.
3. Purge mode (`--purge-aliases`) removes the managed generated files (`generated/bash.sh`, `generated/zsh.sh`, `generated/powershell5.ps1`, `generated/powershell7.ps1`), `aliases.db`, and `operation.journal`. It never deletes a target file referenced by an alias. Directories and backups are left in place.
4. Purge without `--yes` in a non-interactive process fails with `ConfirmationRequired` (exit 14). Retain mode does not require confirmation.

Package-manager hooks and MSI integration are documented in `docs/uninstall.md` and are not implemented in this pre-GUI phase.

## Related documentation

See `README.md`, `docs/security.md`, `docs/limitations.md`, `docs/exit-codes.md`, `docs/uninstall.md`, and `docs/architecture.md`.

GUI commands, visual acceptance, and installer workflows are outside this pre-GUI CLI document.

## GUI notes

- Sync page: **Sync now (generate definitions)** runs the same orchestration as
  `aliasmgr sync`. It lists per-shell success/failure, the generated path, the
  reload command, and whether that shell's loader block is installed. An
  already-open shell must reload; new shells load the file automatically.
- Generated-file preview: if no file exists yet, the UI shows a localized
  "Run Sync to create it" message instead of the raw OS error. Missing,
  unreadable (invalid UTF-8), and permission-denied files are distinguished.
- Config directory: fixed at startup. The Settings page shows a platform
  example (`setx ALIASMGR_CONFIG_DIR "D:\alias-manager"` on Windows,
  `export ALIASMGR_CONFIG_DIR="/home/user/.alias-manager"` on macOS/Linux) and
  refuses to save a different directory. Restart the app after changing the
  environment variable.
- First-run shell integration: when `config.toml` does not exist, the GUI shows
  a panel listing all four shells. Detected/checked shells are previewed
  (profile/RC path plus marked block) before the loader is installed and the
  shell is synced; nothing is enabled without confirmation. Not-detected shells
  offer install-and-restart instructions, manual executable/profile entry, or
  no selection. WSL bash must be integrated inside the distribution or pointed
  at a native Windows executable. **Skip for now** writes an empty
  `shells.enabled`; later launches show only a compact "not configured" notice
  rather than the full wizard.
- Later launches check only enabled shells. A missing/invalid loader shows a
  non-blocking banner with **Re-enable**, **Ignore for now** (session only for
  an invalid loader), and **Disable** (persists `shells.enabled`). Newly detected
  non-WSL shells get a dismissible enable prompt; dismissing it records the
  current executable in `shells.seen`, and disabling a shell does the same, so
  the same installation is not re-offered on every launch. A reinstall at a new
  path prompts once again. Nothing is enabled automatically.
- The Settings page's Shell integration panel supports per-shell enable/disable,
  rescan, loader status, path overrides for non-standard installs, explicit
  "use default" buttons that clear a stored executable/profile override, and
  re-running setup. Per-shell toggles re-read the shared configuration, so a
  shell enabled by another entry point is preserved. The same core
  `detect_shells`/`shell_setup`/`integration` code backs the CLI and GUI.
- GUI create/update is strict: a target shell that is not enabled, or whose
  loader is not valid, blocks the save with a clear message and an "Open shell
  setup" button, and the wizard draft is kept. There is no GUI equivalent of
  `--allow-missing-loader`. The CLI and GUI now call the same core
  `shell_readiness_problems` rule.
- GUI setup results from the startup banner stay visible until acknowledged,
  including loader-install and sync failures. Heavy Tauri commands run off the
  UI thread, and startup status is loaded once and shared with Settings and the
  alias wizard instead of re-probing on mount. If startup status cannot be read,
  the GUI shows a compact retry notice.
- GUI auto-sync runs after a successful database write when
  `runtime.auto_sync = true` (toggle it in Settings): old ∪ new shells for
  create/update, the record's shells for delete/toggle, and all referenced
  shells once for a confirmed import. Per-shell failures, reload commands, and
  missing loader blocks (new terminals will not load the file) are shown;
  import preview never syncs. Auto-sync does not reload the current terminal.
- Shell detection limits are shared with the CLI (PATH, known install paths,
  manual overrides, WSL marking, bounded version probes, home RC defaults).
  `shells.executables`/profile overrides must be absolute; a leading `~` is
  expanded.
