# Limitations

- PowerShell 5.1 and PowerShell 7.0–7.2 may reconstruct native command arguments differently from PowerShell 7.3+; the CLI does not use string evaluation to bypass this host limitation.
- Batch/CMD targets are subject to a second command-line parser; dangerous metacharacters are rejected by the structured executor.
- `.bat` and `.cmd` invoke a second parser and require separate escaping.
- Aliases are not recursively resolved.
- Definitions are intended for interactive shells.
- AllSigned and ConstrainedLanguage environments are not supported by the MVP.
- Alias names are globally unique; different meanings per Shell are not supported.
- Filesystem reliability is conservative: known local Linux filesystems use WAL, known network/remote mounts use DELETE fallback, and unknown or Windows ACL cases remain explicitly unverified rather than being treated as safe.
- Windows ACL writable-path detection is intentionally reported as unknown until a verified native ACL boundary is available; unknown paths must not receive a safety grant.
