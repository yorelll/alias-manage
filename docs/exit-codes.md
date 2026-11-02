# Exit codes

| Code | Meaning | Current reachability |
|---:|---|---|
| 0 | Success | Every successful command |
| 1 | Unclassified/configuration error | `Config` errors, including invalid `--shell`/`--env` values |
| 2 | Usage error | clap rejects unknown arguments, conflicting flags, or malformed command lines |
| 3 | Name or definition conflict | Exact-name, PowerShell case-fold, generic alias conflict, reserved names |
| 4 | Not found | Missing alias in `get`, `update`, `rename`, `enable`, `disable`, `remove` |
| 5 | Validation failure | Invalid name, argument template, advanced-mode/raw Shell rejection, invalid restriction paths |
| 6 | Missing or unsafe target | `UnsafePath` can surface from CLI loader paths; `TargetMissing` exists in the executor but no current CLI path calls it |
| 7 | Permission problem | Reserved in the error mapping; no current producer emits `PermissionDenied` |
| 8 | Lock timeout | Lock acquisition timeout |
| 9 | Syntax check failure | Core-level `SyntaxCheck`; reserved for a future direct command path |
| 10 | Partial synchronization failure | `sync` when any Shell fails, including a syntax-check failure |
| 11 | Rollback failure | Reserved; no `AliasError` variant currently maps to it |
| 12 | Unsupported environment | Newer schema, unsupported/uninstalled Shell |
| 13 | Storage error | Checksum mismatch, SQLite, serialization, I/O, unreliable filesystem |
| 14 | Non-interactive confirmation required | Destructive command without `--yes` in a non-interactive process |

## Implementation mapping

Current `AliasError` mappings in `crates/aliasmgr-cli/src/exit_code.rs`:

- Code 1: `Config` errors, including an unsupported `--shell` or an invalid `KEY=VALUE` environment entry.
- Code 3: `AliasConflict`, `ExactNameConflict`, `CaseFoldConflict`, and `NameReserved`.
- Code 4: `NotFound`, emitted by alias lookup commands when the name does not exist.
- Code 5: `InvalidAliasName`, `InvalidAliasRecord` (empty executable, empty Shell list, or invalid restriction paths such as a relative/foreign-platform path, blank path, NUL, or leading/trailing whitespace), `InvalidArgTemplate`, and `AdvancedModeUnsupported`.
- Code 6: `UnsafePath` and `TargetMissing`. The CLI can produce `UnsafePath` through loader operations; `TargetMissing` is produced by the executor, which no current CLI path calls.
- Code 7: `PermissionDenied` is mapped but has no current producer in the CLI or core paths.
- Code 8: `LockTimeout`.
- Code 9: `SyntaxCheck`. The core maps it to 9, but the current CLI `sync` path collects per-Shell failures and returns `PartialSync`, so users observe exit 10 rather than 9.
- Code 10: `PartialSync`, returned by `sync` when at least one Shell fails.
- Code 12: `SchemaTooNew` and `ShellNotInstalled`.
- Code 13: `ChecksumMismatch`, `Database`, `Serialization`, `Io`, and `UnreliableFilesystem`.
- Code 14: `ConfirmationRequired`, returned by `remove` without `--yes` and by `uninstall --purge-aliases` without `--yes` when stdin is not a TTY.

Code 2 is produced by clap before `AliasError` handling. Code 11 has no `AliasError` variant and remains reserved. Meanings are stable after publication.

The exit-code table describes the CLI contract, not GUI or installer behavior.
