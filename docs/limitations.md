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
