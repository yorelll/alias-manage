# CLI Commands

This document describes the current pre-GUI CLI surface. It is a command contract verified against `crates/aliasmgr-cli` and the core library on 2026-11-01, not a claim that every environment-dependent Shell behavior is accepted on every machine. CI and tests always use an isolated temporary configuration directory.

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

## Alias lifecycle

- `add <NAME> --exec <PROGRAM> [--arg VALUE]... [--shell SHELL]... [--cwd PATH] [--env KEY=VALUE]... [--tag TAG]... [--description TEXT] [--restrict MODE] [--restrict-path PATH]... [--no-pass-args] [--target-type TYPE] [--path-mode MODE]`
  - `--arg` is repeatable and accepts leading hyphens, so `--arg --oneline` is valid. Because hyphen values are allowed, a following flag can be consumed as a value when it is ambiguous; use `--arg=--oneline` or place other flags before `--arg` in that case.
  - `--shell` accepts `bash`, `zsh`, `powershell5`, and `powershell7`, and is repeatable. When omitted, the alias targets Bash only.
  - `--env KEY=VALUE` is repeatable and is stored on the record. A missing `=`, an empty key, or an invalid shell is a configuration error (exit 1).
  - `--cwd PATH` applies to each generated invocation without changing the caller's shell: Bash/Zsh run the command in a subshell after `cd`; PowerShell uses `Push-Location` and restores it in `finally`.
  - `--env` values apply only to the generated invocation: Bash/Zsh invoke the target through `env KEY=VALUE ...`; PowerShell saves, sets, and restores each variable in `finally`.
  - `--no-pass-args` sets `pass_args=false`; user arguments are forwarded by default. When fixed arguments contain `{{args}}`, user arguments are inserted at that position; `{{{{args}}}}` renders the literal text `{{args}}`. Without a placeholder, user arguments are appended at the end only when `pass_args` is true.
  - `--target-type TYPE` accepts `native_executable` (default), `python_script`, `powershell_script`, `batch`, `shell_script`, `java_jar`, `cmdlet`, and `change_directory`. `raw_shell_command` is not exposed as a CLI value and is rejected by clap as a usage error (exit 2); advanced Shell mode remains rejected by validation.
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
- `update <NAME> [--exec <PROGRAM>] [--arg VALUE]... [--shell SHELL]... [--cwd PATH] [--env KEY=VALUE]... [--tag TAG]... [--description TEXT] [--restrict MODE] [--restrict-path PATH]... [--clear-restriction] [--no-pass-args|--pass-args] [--target-type TYPE] [--path-mode MODE]`
  - Only supplied fields change. Omitted fields keep their stored values.
  - `--arg`, `--shell`, and `--tag` replace their whole collection when supplied. `--env` merges keys into the existing environment map. `--cwd` replaces the stored working directory; there is currently no flag to clear it.
  - `--description` replaces the stored memo; `--description ""` clears it.
  - Restriction updates: `--restrict` alone changes only the mode and keeps existing paths (error if the record has no restriction and no paths are supplied); `--restrict-path` alone replaces the path list and keeps the existing mode (error if the record has no restriction); supplying both replaces both; `--clear-restriction` removes the restriction. Omitted restriction flags preserve the stored value. Validation and path rules match `add`. `--clear-restriction` conflicts with `--restrict` and `--restrict-path` (clap usage error, exit 2). See `docs/restrictions.md`.
  - Removing a shell from the record retires the old name for that shell so the next `sync` cleans it up.
  - `--target-type` and `--path-mode` accept the same values as `add` and replace the stored values when supplied; omitted values are preserved.
  - A missing alias is `NotFound` (exit 4). An invalid shell or environment entry is a configuration error (exit 1). `--no-pass-args` and `--pass-args` conflict and are rejected by clap (exit 2). An unsupported `--target-type` or `--path-mode` value is a clap usage error (exit 2).
- `rename <OLD> <NEW>`: retires the old name for every bound shell and updates the record. A missing source is `NotFound` (exit 4); a name conflict is exit 3.
- `enable <NAME>` / `disable <NAME>`: flips the `enabled` flag. A missing alias is `NotFound` (exit 4).
- `remove <NAME> [--yes]`: retires the name for every bound shell (tombstones advance the revision) and deletes the row. A missing alias is `NotFound` (exit 4). Without `--yes` in a non-interactive process, the command fails with `ConfirmationRequired` (exit 14).

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
- `doctor`: reports missing database, missing generated files, stale generated files (recorded `applied_revision` behind the current revision), recorded sync errors, missing loader blocks, and a filesystem-reliability finding when one is available. It also recomputes each generated file's embedded `# file_checksum:` and reports `生成文件校验和不匹配（可能被手工修改）` for a file edited by hand or missing its checksum line. It does not repair anything.
- `shell detect`: reports the detected Shell.
- `shell install <SHELL>` / `shell uninstall <SHELL>`: installs or removes the marked loader block for `bash`, `zsh`, `powershell5`, or `powershell7`. Other Shell names return `ShellNotInstalled` (exit 12).
  - The loader path comes from `config.toml` (`shells.bash_rc_path`, `shells.zsh_rc_path`, `shells.powershell5_profile_path`, `shells.powershell7_profile_path`).
  - Bash/Zsh default to `<config-root>/.bashrc` / `<config-root>/.zshrc`.
  - On Windows, PowerShell 5.1 defaults to `%USERPROFILE%\Documents\WindowsPowerShell\Microsoft.PowerShell_profile.ps1` and PowerShell 7 to `%USERPROFILE%\Documents\PowerShell\Microsoft.PowerShell_profile.ps1`.
  - Outside Windows, PowerShell 7 defaults to `~/.config/powershell/profile.ps1`; PowerShell 5.1 without a configured path returns `ShellNotInstalled`, because that Shell does not exist there.
  - Default paths are derived from `%USERPROFILE%` / `~`; OneDrive-redirected Documents folders are not detected through the Windows known-folder API, so set the corresponding `shells.powershell*_profile_path` explicitly in that case.
  - The PowerShell block dot-sources the generated `.ps1` only when it exists and then removes its temporary variable; a missing generated file cannot break profile startup. Profile writes create a backup and respect the configured RC retention limit.

## Transfer

- `export <FILE>`: writes JSON by default, or TOML when the path ends in `.toml`. Environment keys whose names contain `PASSWORD`, `TOKEN`, `SECRET`, `API_KEY`, or `APIKEY` (case-insensitive) are omitted from the export. The same envelope is the single import/export format for Bash, Zsh, PowerShell 5.1, and PowerShell 7: each record carries its `shells` list, plus `description`, `tags`, and an optional `restriction`. Importing into an empty database preserves those fields.
- `import <FILE> [--yes]`
  - The import file is parsed and every candidate record is validated with the same rules as `add` (alias-name syntax, PowerShell-reserved names, argument templates, restriction paths). Restriction paths are trimmed, blank entries dropped, and duplicates removed before validation and persistence. Records that fail validation are skipped with a warning and are never persisted; a record whose restriction path is absolute only on another platform is skipped wholesale rather than imported without its restriction. Sensitive environment keys are filtered on import as well.
  - Without `--yes`, the command previews only: it opens the existing database read-only, evaluates conflicts through `ConflictStrategy::Ask`, and reports `imported`, `skipped`, `unsupported`, and `warnings` counts without persisting anything. Each warning is printed to stderr (for example an omitted sensitive-key name or a rejected invalid record); warning text never includes environment values, and echoed names/keys have control characters stripped and are truncated for display. The read-only SQLite handle may leave `-wal` / `-shm` side files next to the database.
  - With `--yes`, accepted records are persisted atomically in one transaction. Existing-name conflicts are reported as skipped without a warning; records rejected by validation are reported as skipped with a warning. Neither is persisted. Unsupported records are counted and are never persisted.
  - Preview cannot model every insert-time conflict. For example, PowerShell case-fold persistence can reject or rename a record when it is actually written.

## Uninstall

`uninstall [--purge-aliases] [--yes]`

1. Loader cleanup runs first. The command removes marked loader blocks from the configured Bash/Zsh RC and PowerShell profile paths, plus `<config-root>/.bashrc` and `<config-root>/.zshrc` when those files exist. A profile is rewritten only when a block was removed, with a backup and the configured RC retention limit. A malformed or unpaired marker aborts before any destructive step.
2. Retain mode (default) keeps `generated/` and `aliases.db`. Generated files are self-contained and do not require the `aliasmgr` program at Shell startup.
3. Purge mode (`--purge-aliases`) removes the managed generated files (`generated/bash.sh`, `generated/zsh.sh`, `generated/powershell5.ps1`, `generated/powershell7.ps1`), `aliases.db`, and `operation.journal`. It never deletes a target file referenced by an alias. Directories and backups are left in place.
4. Purge without `--yes` in a non-interactive process fails with `ConfirmationRequired` (exit 14). Retain mode does not require confirmation.

Package-manager hooks and MSI integration are documented in `docs/uninstall.md` and are not implemented in this pre-GUI phase.

## Related documentation

See `README.md`, `docs/security.md`, `docs/limitations.md`, `docs/exit-codes.md`, `docs/uninstall.md`, and `docs/architecture.md`.

GUI commands, visual acceptance, and installer workflows are outside this pre-GUI CLI document.
