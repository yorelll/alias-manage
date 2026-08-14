# GUI Testing Manual Design

## Information architecture

The GUI manual will live under `docs/gui/`:

```text
docs/gui/
├── README.md
├── linux.md
├── windows.md
├── test-matrix.md
├── report-template.md
└── troubleshooting.md
```

`README.md` is the entry point and explains scope, preparation, evidence, result statuses, and CI/manual boundaries. `linux.md` and `windows.md` contain platform-specific procedures. `test-matrix.md` owns stable test IDs and expected outcomes. `report-template.md` is the user-facing record form. `troubleshooting.md` provides diagnosis paths.

## Test flow

GUI acceptance follows a fixed sequence: preparation, basic lifecycle, high-risk behavior, cleanup and uninstall. Tests use a clean user or dedicated temporary configuration directory and must not silently consume or modify the user's real profiles.

The basic lifecycle is: launch, version check, empty list, create, edit, search, sort, enable/disable, details/preview, sync, reload, delete, and residual-session messaging. High-risk cases cover spaces, CJK, quotes, empty arguments, backslashes, wildcards, `{{args}}`, working directories, environment variables, name preemption, PowerShell built-ins, manual generated-file edits, sync failure/recovery, import safety, and uninstall retention/purge.

## Evidence and result rules

Each test records version, package SHA256, OS, architecture, GUI build, Shell/version, start time, config directory, and clean-environment status. Evidence may be screenshots, recordings, Shell output, doctor output, sanitized file summaries, database summaries, CI run IDs, or reproduction logs. Secrets and sensitive variables must be redacted; complete profiles must not be uploaded.

Allowed results are `PASS`, `FAIL`, `BLOCKED`, `EXPECTED-LIMITATION`, and `NOT-APPLICABLE`. Core acceptance requires all golden-path cases to pass, unexplained failures to be absent, blocked cases to have explicit environment causes, limitations to match `docs/limitations.md`, and uninstall/recovery to preserve unmanaged content.

## Platform matrix

Linux procedures cover Ubuntu 24.04, Tauri/WebKitGTK, Bash `.bashrc`, Zsh `.zshrc`, login/non-interactive behavior, oh-my-zsh ordering, RC symlinks, LF endings, `bash -n`, `zsh -n`, permissions, other-writable targets, and isolated HOME/XDG paths.

Windows procedures cover Windows 10/11, WebView2, PowerShell 5.1 and 7 independently, Profile discovery and OneDrive redirection, BOM/CRLF, Restricted/AllSigned/Group Policy/ConstrainedLanguage, ACLs, EXE/BAT/CMD/PS1 targets, built-in alias preemption, and isolated USERPROFILE/LOCALAPPDATA/APPDATA paths.

## CI versus manual boundary

CI owns Rust compilation, core/CLI tests, shell adapter tests, argument matrices, SQLite migration/journal/sync/transfer/uninstall tests, and Tauri compilation/frontend tests. Users own GUI visual/interaction quality, real profiles, enterprise policy, actual terminal behavior, oh-my-zsh/dotfiles compatibility, symlink behavior on real machines, and install/upgrade/uninstall experience.

## Report format

Each case records a stable ID, result, version, OS, Shell, package, config path, preconditions, numbered steps, expected behavior, actual behavior, evidence links, CI run, sanitization check, and notes.
