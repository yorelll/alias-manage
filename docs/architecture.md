# Architecture

Alias Manager is a Rust workspace. `aliasmgr-core` owns the domain model, validation, storage, execution and shell adapters; CLI and GUI layers call the core API.

The current pre-GUI CLI boundary targets Bash, Zsh, PowerShell 5.1 and PowerShell 7. Tauri 2 Linux builds require WebKitGTK 4.1 and related GTK dependencies; GUI compilation and interaction are a separate later gate. CI uses Ubuntu 24.04 for core checks. Linux packaging targets Ubuntu 22.04 for glibc compatibility.

SQLite with the bundled feature is the configuration source of truth. Shell files are rebuildable derived artifacts.

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
