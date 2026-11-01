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

- `add <NAME> --exec <PROGRAM> [--arg VALUE]... [--shell SHELL]... [--cwd PATH] [--env KEY=VALUE]... [--tag TAG]... [--no-pass-args]`
  - `--arg` is repeatable and accepts leading hyphens, so `--arg --oneline` is valid. Because hyphen values are allowed, a following flag can be consumed as a value when it is ambiguous; use `--arg=--oneline` or place other flags before `--arg` in that case.
  - `--shell` accepts `bash`, `zsh`, `powershell5`, and `powershell7`, and is repeatable. When omitted, the alias targets Bash only.
  - `--env KEY=VALUE` is repeatable and is stored on the record. A missing `=`, an empty key, or an invalid shell is a configuration error (exit 1).
  - `--no-pass-args` sets `pass_args=false`; user arguments are forwarded by default.
  - Advanced Shell mode and raw Shell commands are rejected by validation and are not exposed as CLI flags.
- `get <NAME>`: prints the stored record. A missing alias is `NotFound` (exit 4).
- `list [--sort FIELD] [--desc] [--limit N] [--tag TAG]...`
  - `--sort` accepts `name` (default), `updated_at`, `created_at`, `target_type`, and `enabled`.
  - `--limit` defaults to 50. Repeated `--tag` values use AND intersection.
- `find <QUERY> [--fuzzy] [--field FIELD] [--limit N] [--tag TAG]...`
  - `--field` accepts `all` (default), `name`, `target`, `description`, `tags`, `shells`, and `target_type`.
  - `--fuzzy` adds subsequence matching on top of exact/prefix/substring scoring. Without it, exact, prefix, and substring matches still score; there is no separate exact-only mode.
- `update <NAME> [--exec <PROGRAM>] [--arg VALUE]... [--shell SHELL]... [--cwd PATH] [--env KEY=VALUE]... [--tag TAG]... [--no-pass-args|--pass-args]`
  - Only supplied fields change. Omitted fields keep their stored values.
  - `--arg`, `--shell`, and `--tag` replace their whole collection when supplied. `--env` merges keys into the existing environment map. `--cwd` replaces the stored working directory; there is currently no flag to clear it.
  - Removing a shell from the record retires the old name for that shell so the next `sync` cleans it up.
  - A missing alias is `NotFound` (exit 4). An invalid shell or environment entry is a configuration error (exit 1). `--no-pass-args` and `--pass-args` conflict and are rejected by clap (exit 2).
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
  - `--dry-run` is a non-mutating no-op success. It does not create the database, generated files, or backups.
- `reload [--print] [--shell <SHELL>]`
  - Without `--print`, prints the reminder that an already-open parent Shell is not updated by a child process.
  - With `--print`, prints the Shell-specific command that sources the generated file. `--shell` accepts `bash`, `zsh`, `powershell5`, and `powershell7`; when omitted, the Shell is detected and falls back to Bash. An unknown explicit Shell is a configuration error (exit 1).
  - `reload` never modifies files or Shell state by itself.
- `doctor`: reports missing database, missing generated files, stale generated files (recorded `applied_revision` behind the current revision), recorded sync errors, missing loader blocks, and a filesystem-reliability finding when one is available. It does not repair anything.
- `shell detect`: reports the detected Shell.
- `shell install <SHELL>` / `shell uninstall <SHELL>`: installs or removes the marked loader block for `bash` or `zsh`. The loader path comes from `config.toml` or defaults to `<config-root>/.bashrc` / `<config-root>/.zshrc`. Other Shell names return `ShellNotInstalled` (exit 12). Profile writes create a backup and respect the configured RC retention limit.

## Transfer

- `export <FILE>`: writes JSON by default, or TOML when the path ends in `.toml`. Environment keys whose names contain `PASSWORD`, `TOKEN`, `SECRET`, `API_KEY`, or `APIKEY` (case-insensitive) are omitted from the export.
- `import <FILE> [--yes]`
  - The import file is parsed and validated. Sensitive environment keys are filtered on import as well.
  - Without `--yes`, the command previews only: it opens the existing database read-only, evaluates conflicts through `ConflictStrategy::Ask`, and reports `imported`, `skipped`, and `unsupported` counts without persisting anything. The read-only SQLite handle may leave `-wal` / `-shm` side files next to the database.
  - With `--yes`, accepted records are persisted atomically in one transaction. Existing-name conflicts are reported as skipped. Unsupported records are counted and are never persisted.
  - Preview cannot model every insert-time conflict. For example, PowerShell case-fold persistence can reject or rename a record when it is actually written.

## Uninstall

`uninstall [--purge-aliases] [--yes]`

1. Loader cleanup runs first. The command removes marked loader blocks from the configured Bash/Zsh RC and PowerShell profile paths, plus `<config-root>/.bashrc` and `<config-root>/.zshrc` when those files exist. A profile is rewritten only when a block was removed, with a backup and the configured RC retention limit. A malformed or unpaired marker aborts before any destructive step.
2. Retain mode (default) keeps `generated/` and `aliases.db`. Generated files are self-contained and do not require the `aliasmgr` program at Shell startup.
3. Purge mode (`--purge-aliases`) removes the managed generated files (`generated/bash.sh`, `generated/zsh.sh`, `generated/powershell5.ps1`, `generated/powershell7.ps1`), `aliases.db`, and `operation.journal`. It never deletes a target file referenced by an alias. Directories and backups are left in place.
4. Purge without `--yes` in a non-interactive process fails with `ConfirmationRequired` (exit 14). Retain mode does not require confirmation.

Package-manager hooks and MSI integration are documented in `docs/uninstall.md` and are not implemented in this pre-GUI phase.

## Related documentation

See `README.md`, `docs/security.md`, `docs/testing.md`, `docs/limitations.md`, `docs/exit-codes.md`, `docs/ci-workflow.md`, `docs/release-checklist.md`, and `docs/phase4-reconciliation.md`.

GUI commands, visual acceptance, and installer workflows are outside this pre-GUI CLI document.
