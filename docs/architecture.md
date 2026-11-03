# Architecture

Alias Manager is a Rust workspace. `aliasmgr-core` owns the domain model, validation, storage, execution and shell adapters; the CLI, TUI and GUI layers call the core API.

The current pre-GUI CLI boundary targets Bash, Zsh, PowerShell 5.1 and PowerShell 7. Tauri 2 Linux builds require WebKitGTK 4.1 and related GTK dependencies; GUI compilation and interaction are a separate later gate. CI uses Ubuntu 24.04 for core checks. Linux packaging targets Ubuntu 22.04 for glibc compatibility.

SQLite with the bundled feature is the configuration source of truth. Shell files are rebuildable derived artifacts. The workspace pins `rusqlite 0.41.1` / `libsqlite3-sys 0.39.1`, bundling SQLite 3.54.1. SQLite versions before 3.51.3 contain the documented WAL-reset corruption bug when multiple connections write/checkpoint in separate threads or processes; those versions are not supported. The TUI subset retry also reuses the job's existing database connection for shell-state writes to keep the live connection count as low as practical.

CLI, the TUI (`crates/aliasmgr-tui`) and the Tauri GUI resolve the configuration root through the same `AppPaths::discover` and open the same `<config-root>/aliases.db`. No frontend keeps its own database or a second copy of record state; there is exactly one SQLite file per configuration root.

## TUI layer

`crates/aliasmgr-tui` is an optional dependency of `aliasmgr-cli` behind the
default-on `tui` feature. It depends on `aliasmgr-core` only (never on the CLI
or GUI), and adds just `ratatui`, `crossterm` and `unicode-width`. With
`--no-default-features` the CLI builds without these crates.

The TUI is a presentation and input-state layer:

- Reads and writes go through `AliasService` / `Database`. Optimistic
  concurrency uses `AliasService::{update_checked, delete_checked,
  rename_checked, set_enabled_checked}`, which add a revision guard to the
  database `UPDATE`/transaction. Those transactions use `BEGIN IMMEDIATE` so a
  concurrent commit waits instead of surfacing `SQLITE_BUSY_SNAPSHOT`. A rename or a removed shell retires the old
  name through the same tombstone rules as the CLI.
- Sync uses `sync::sync_config_dir_collect_io`, so per-shell results, the
  monotonic revision, lock, backups, journal and atomic writes stay in core.
  Sync and Doctor actions run on a `std::thread` and report through an `mpsc`
  channel; one job writes at a time and the UI never blocks. The job runner also
  understands loader install/uninstall results, but this version exposes no TUI
  action that starts them.
- Doctor uses `doctor::diagnose`; loader status probing uses `integration`
  (install/uninstall plumbing exists in the job runner but has no TUI action);
  previews use `SyncCoordinator::render_shell`; reload commands come from
  `sync::reload_command`.
- At startup an interrupted sync journal is recovered through
  `SyncCoordinator::recover_pending_operations` under the shared sync lock;
  the TUI does not implement its own recovery.
- The event loop uses crossterm `poll(250ms)` and a dirty flag; there is no
  timer-driven redraw. Raw mode and the alternate screen are owned by an RAII
  session and a panic hook chains a best-effort restore before the previous
  hook.
- The TUI re-reads the database on every refresh and polls a cheap SQLite
  fingerprint for external CLI/GUI writes. It never treats its cached snapshot
  as the source of truth.

## Data model and storage

`AliasRecord` carries `description`, `tags`, and an optional `restriction` (`PathRestriction { mode: whitelist|blacklist, paths }`) in addition to the execution fields. The SQLite `aliases` table stores `description` and `tags_json` directly; `restriction_json` stores the serialized restriction or the literal JSON `null`. `serde` treats `restriction` as optional (`#[serde(default)]`, skipped when `None`), so records written before the field existed keep their `record_checksum` unchanged and still import.

## Migrations

`migrations::migrate` applies ordered `(version, sql)` pairs strictly newer than `PRAGMA user_version` inside one transaction, records each applied version in `schema_migrations`, then advances `user_version`. `CURRENT_VERSION` is 2:

- `0001_initial.sql`: base tables and indexes.
- `0002_restriction.sql`: `ALTER TABLE aliases ADD COLUMN restriction_json TEXT NOT NULL DEFAULT 'null'`.

A v1 database therefore runs only `0002`; a fresh database runs both. `SchemaTooNew` is preserved for databases ahead of `CURRENT_VERSION`.

## Restriction rendering

Restrictions are enforced by generated wrapper code, not by the CLI at runtime:

- `common::is_simple_alias` and `powershell::is_simple_alias` return false when a restriction is present, so the alias is always a wrapper function rather than a native `alias`/`Set-Alias`.
- Bash and Zsh emit the same guard at the top of every wrapper (including `change_directory`), before the command, working-directory subshell, and environment subshell. The guard resolves the invocation directory with `pwd -P`, canonicalizes each configured directory by `cd`-ing into it in a subshell, skips missing directories, and compares at component boundaries (`dir` or `dir/*`), treating a filesystem root as matching everything. A configured directory that resolves to `/` after `cd` + `pwd -P` is treated as a universal root even when Git Bash substituted its MSYS install path for the original `/` argument. On Git Bash/Cygwin it additionally runs both values through `cygpath -m` and compares case-insensitively; if `tr` fails, the comparison falls back to the original non-empty values. Whitelist denies when nothing matched; blacklist denies when anything matched. All state uses `local __aliasmgr_*` variables, paths are single-quoted with `quote_posix`, and no `eval` is used.
- PowerShell emits the equivalent guard inside `function global:<name>` before `Push-Location` and environment changes. A shared `__aliasmgr_canonical_path` helper is emitted once per generated file: it resolves symlinks/junctions with `DirectoryInfo.ResolveLinkTarget` on .NET 6+/PowerShell 7, falls back to a Win32 `GetFinalPathNameByHandleW` P/Invoke on Windows PowerShell 5.1, and falls back to the lexical full path when neither works. The guard refuses non-FileSystem locations such as `HKLM:\`, trims trailing separators, and compares full paths with `OrdinalIgnoreCase` on Windows and `Ordinal` elsewhere. Restricted wrappers are advanced functions (`[CmdletBinding()]`, `ValueFromRemainingArguments`) so `$PSCmdlet.WriteError` makes `$?` false; `$global:LASTEXITCODE = 1` is set before the error is written, so `-ErrorAction Stop` still observes 1. No `Invoke-Expression` is used.

## Transfer format

One JSON/TOML envelope (`format_version`, `exported_at`, `exported_by_version`, `aliases`) is the single import/export format for Bash, Zsh, PowerShell 5.1, and PowerShell 7. Each record lists its target `shells`; shell-specific definitions are regenerated from the record rather than stored in the file. `restriction` travels with the record and is revalidated on import.
