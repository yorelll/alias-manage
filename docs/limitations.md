# Limitations

- PowerShell 5.1 and PowerShell 7.0–7.2 may reconstruct native command arguments differently from PowerShell 7.3+; the CLI does not use string evaluation to bypass this host limitation. Exact argv preservation is guaranteed at the Alias Manager structured-array boundary; host-native reconstruction remains an expected limitation for affected shapes.
- The argument boundary matrix covers empty strings, spaces, quotes, backslashes, Unicode, wildcards, fixed/user/middle `{{args}}`, working-directory/environment metadata, Python/PowerShell/JAR/native target types, and Batch/CMD metacharacter rejection. Fixtures are CI-only and do not print environment secrets.
- Batch/CMD targets are subject to a second command-line parser; dangerous metacharacters are rejected by the structured executor rather than reinterpreted through a shell string.
- `.bat` and `.cmd` invoke a second parser and require separate escaping.
- Aliases are not recursively resolved.
- Definitions are intended for interactive shells.
- AllSigned and ConstrainedLanguage environments are not supported by the MVP.
- Alias names are globally unique; different meanings per Shell are not supported.
- Filesystem reliability is conservative: known local Linux filesystems use WAL, known network/remote mounts use DELETE fallback, and unknown or Windows ACL cases remain explicitly unverified rather than being treated as safe.
- Windows ACL writable-path detection is intentionally reported as unknown until a verified native ACL boundary is available; unknown paths must not receive a safety grant.
- Syntax checking is best effort. Generated Bash/Zsh files are checked with `bash -n` / `zsh -n` and PowerShell files with the host parser only when that interpreter is present and usable (a health probe must succeed). If the interpreter is missing or unusable (for example a WSL launcher stub on Windows), the check is a documented no-op and the file is accepted unvalidated. Passing a syntax check does not prove that an alias becomes effective in a live Shell.
- PowerShell parser diagnostics are locale-dependent. The syntax probe forces UTF-8 console output to avoid mojibake, but the message language follows the host UI culture and may appear in en-US or another locale. Tests assert readable text, not a specific localized wording.
- `--no-color` and `--verbose` are accepted but currently reserved no-ops. `get` always prints a Rust debug representation and ignores `--format`.
- `update` can set or replace `--cwd`, environment keys, and collections, but it cannot clear a working directory or remove an individual environment key.
- `import` preview opens a read-only SQLite handle against an existing database. That handle may leave `-wal` / `-shm` side files next to `aliases.db` even though no alias is persisted.
- Import preview cannot model every insert-time conflict. For example, `ConflictStrategy::Overwrite` case-fold persistence in PowerShell can behave differently when records are actually written, so preview counts are not a guarantee of the `--yes` outcome.
- The current revision is derived from alias rows and tombstone rows (`max(alias revisions, tombstone revisions, 1)`). If tombstone pruning is wired in later without a stored monotonic counter, the derived revision could regress relative to an already-applied revision.
- GUI visual layout, real-window interaction, keyboard focus, accessibility, and real terminal behavior remain manual acceptance items. Frontend unit tests, type checking, and a successful build do not prove those qualities.
