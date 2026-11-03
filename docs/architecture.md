# Architecture

Alias Manager is a Rust workspace. `aliasmgr-core` owns the domain model, validation, storage, execution and shell adapters; the CLI, TUI and GUI layers call the core API.

The current pre-GUI CLI boundary targets Bash, Zsh, PowerShell 5.1 and PowerShell 7. Tauri 2 Linux builds require WebKitGTK 4.1 and related GTK dependencies; GUI compilation and interaction are a separate later gate. CI uses Ubuntu 24.04 for core checks. Linux packaging targets Ubuntu 22.04 for glibc compatibility.

SQLite with the bundled feature is the configuration source of truth. Shell files are rebuildable derived artifacts. The workspace pins `rusqlite 0.41.1` / `libsqlite3-sys 0.39.1`, bundling SQLite 3.54.1. SQLite 3.7.0–3.51.2 without the 3.44.6/3.50.7 backports contain the documented WAL-reset corruption bug when multiple connections write/checkpoint in separate threads or processes. This project requires/bundles >=3.51.3 (currently 3.54.1). The TUI subset retry also reuses the job's existing database connection for shell-state writes to keep the live connection count as low as practical.

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
  channel; one job writes at a time and the UI never blocks. After a successful
  create/update/delete/rename/enable-disable, `runtime.auto_sync = true` starts
  one `AutoSync` job for the affected shells (old ∪ new for updates); a
  confirmed import starts one full `AutoSyncAll`. Auto-sync reuses the same Jobs
  worker and core sync lock, refuses to start while another job is running, and
  reports per-shell results without rolling back the database write.
- Doctor uses `doctor::diagnose`; loader status probing uses `integration`;
  previews use `SyncCoordinator::render_shell`; reload commands come from
  `sync::reload_command`.
- Shell integration is the shared `shell_setup` state machine in
  `crates/aliasmgr-tui/src/shell_setup_ui.rs`. `run_shell_setup` runs it
  standalone for interactive `aliasmgr doctor`; the `aliasmgr tui` app embeds
  the same `SetupApp` as `Screen::ShellSetup` for first-run, startup repair, the
  settings panel and the strict add/edit preflight. Loader/marker/backup/sync
  policy stays in core; the TUI only drives detection rows, selections, preview
  and confirmation. Startup detection runs on a read-only `mpsc` worker, not on
  the Jobs writer, so the browser stays usable while versions probe.
- The editor's shell labels (`enabled`/`disabled`/`not installed`/loader
  invalid) are display projections of cached detection + live loader status.
  Save runs `shell_setup::shell_readiness_problems` before the database write;
  a blocked save keeps the draft and suspends the editor while the setup screen
  is open, then restores it.
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
- The same crate exposes `run_shell_setup`, the interactive `aliasmgr doctor`
  Shell integration screen. It is reached from the CLI when the `tui` feature
  is enabled and gives the
  `--no-default-features` build a line-based fallback instead of crossterm.

## Shell detection and setup

`aliasmgr-core` owns shell integration policy so the CLI, TUI and GUI cannot
drift:

- `detection::detect_shells(&AppConfig)` returns all four supported shells with
  executable, detected flag, source (`manual`, `path`, `known_path`, `wsl`,
  `not_found`), optional version and a WSL marker. Manual
  `shells.executables` entries win over `PATH`, which wins over known install
  paths. Probes are read-only and bounded by a short timeout; a failed probe
  only leaves the version unknown.
- `config::ShellPathOverrides` persists the user's `enabled` intent using the
  stable `ShellKind::as_str()` spellings, plus the existing profile overrides,
  the optional `shells.executables` map, and `shells.seen` (last dismissed
  executable per shell so startup prompts do not repeat until the path
  changes). Loading a legacy configuration leaves `enabled`/`seen` empty;
  nothing is enabled without an explicit user action.
- `shell_setup` owns `enabled_shells`, `set_enabled_shells`,
  `set_shell_enabled` (re-reads config and toggles one shell, preserving
  concurrent enables), `mark_shells_seen`, `plan_shell_setup` (read-only plan
  with loader target/status and next action), `preview_loader` (block and path
  without writing), and `apply_shell_setup` (persist intent/overrides, install
  via `integration`, sync via `sync`). Bash/Zsh loader defaults come from the
  user home (`$HOME/.bashrc` / `.zshrc`), not the config root; the old
  config-root RC is retained only as an uninstall candidate. Override paths are
  expanded from a leading `~` and must be absolute; the `CLEAR_OVERRIDE`
  sentinel removes a stored executable/profile override and falls back to
  defaults. It reuses the existing loader marker, backup, lock, journal and
  revision code instead of duplicating it.
- The interactive doctor previews every shell that will be applied and applies
  exactly that previewed set; a checked shell that still needs a path blocks
  the confirmation instead of being written un-previewed.
- `shell_readiness_problems` is the shared preflight rule: a target shell must
  be in `shells.enabled` and its loader block must verify. Both the GUI save
  commands and the CLI `add`/`update` call the same core function (the CLI uses
  `shell_readiness_problems_with(..., check_loader = !no_sync)` and keeps its
  `ShellNotReady` exit 12 / `--allow-missing-loader` behavior). The GUI is
  strict: a blocked save writes nothing and keeps the draft.
- The GUI (`crates/aliasmgr-gui/src-tauri/src/commands.rs`) exposes read-only
  `shell_status` / `shell_startup_state`, plan/preview `shell_setup_plan`,
  confirmed `shell_setup_apply`, per-shell `shell_set_shell_enabled`,
  `shell_set_enabled`, `shell_dismiss_detected`, `shell_setup_skip` and
  `preflight_alias_shells`. Heavy commands use `#[tauri::command(async)]` so
  detection, CRUD and sync run off the UI thread. `shell_startup_state.first_run`
  is true only when `config.toml` is absent; skipping writes an empty
  configuration so later launches show a compact notice instead of the full
  wizard. Startup reports `invalid_enabled` (enabled but missing executable or
  loader invalid) and `newly_detected` (detected, not enabled, non-WSL, and not
  dismissed at the current executable path) without writing anything.
- Each `shell_setup_apply`/`shell_setup_plan` call delegates path, marker,
  backup, lock and journal handling to `integration`/`sync`; the GUI never
  edits profile text itself. The status scan reports probed versions; previews
  reuse the shared planner and do not add a version field of their own.
- GUI writes use the same post-commit auto-sync contract as the CLI:
  `create`/`update` sync old ∪ new shells, `delete` syncs the removed record's
  shells (tombstones), and confirmed imports sync all referenced shells once.
  `update_alias` honors the stored revision and retires removed shells through
  `AliasService::update_checked`. Per-shell failures, orchestration errors and
  missing loader blocks (`loader_installed = false`) are returned in
  `AutoSyncDto` so the UI can warn that new terminals will not load the file;
  preview never syncs. The App loads startup shell status once and passes those
  rows to the Settings panel and alias wizard, which only fetch on demand when
  no rows are available.

After a successful CLI alias write, `commands::sync_after_write` computes the
affected shells (old ∪ new for `add`/`update`, bound shells for
`remove`/`rename`/`enable`/`disable`) and calls `sync::sync_shells` once when
`runtime.auto_sync` is true. A confirmed import syncs all referenced shells once
through `sync::sync_config_dir`. A per-shell or orchestration failure is
reported as `PartialSync` (exit 10) after the database commit; the generated
file and `shell_state` remain the source of truth for diagnostics.

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
