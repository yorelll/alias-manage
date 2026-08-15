# Pre-GUI CLI MVP Release Checklist

This checklist defines the non-GUI release boundary. It does not certify the Tauri GUI, installers, package hooks, visual behavior, or real-user profile compatibility.

## CI evidence

Current pre-GUI documentation and CLI boundary are supported by the repository files listed in this checklist; the commands below are the authoritative remote verification runs.

- [x] Fast CI run `31875233532` passed Linux tests, Windows tests, and lint.
- [x] Integration run `31875307387` passed Bash, Zsh, PowerShell 5.1/7, lock, CLI-layer, and isolation jobs.
- [x] Task-specific CI evidence is recorded beside the relevant Task 1–17 subordinate items in `plan.md`.
- [ ] GUI build and frontend test jobs are intentionally outside this pre-GUI gate.

## CLI/core scope

- [x] SQLite is the configuration source of truth; generated shell files are derived artifacts.
- [x] Structured argv execution is used in ordinary mode.
- [x] Advanced shell mode and `RawShellCommand` remain rejected.
- [x] Bash/Zsh and PowerShell adapter behavior has CI evidence.
- [x] Import/export safety filters sensitive environment keys and rejects unsupported records.
- [x] Uninstall protects referenced target files.
- [x] Existing parent Shell sessions are not claimed to update automatically; reload guidance is required.
- [ ] Durable SQLite shell-state transaction binding and full forward recovery remain incomplete.

## Documentation boundary

- [x] README identifies the CI-only Rust verification boundary and non-GUI limitations.
- [x] Architecture, security, testing, limitations, exit-code, CI, and uninstall documents exist.
- [x] Exit-code meanings are documented as stable after publication; unused future codes remain reserved.
- [x] Environment-dependent limitations are documented: PowerShell profiles/ExecutionPolicy, symlinked RC/Profile files, oh-my-zsh ordering, real terminal behavior, and GUI interaction.
- [x] Package-manager and Windows MSI integration boundaries are documented in `docs/uninstall.md`.

## Manual acceptance still required

- [ ] Clean Linux user-directory install/create/reload/delete/upgrade/uninstall flow.
- [ ] Clean Windows profile and PowerShell 5.1/7 install/create/reload/delete/upgrade/uninstall flow.
- [ ] Real user profile backups, symlink behavior, OneDrive redirection, ExecutionPolicy, and plugin ordering.
- [ ] Package-manager hooks and Windows installer behavior.
- [ ] GUI startup, visual presentation, interaction, and cross-platform acceptance.

## Security review

- [ ] Search ordinary generated paths for `eval` and `Invoke-Expression`; `reload --print` is the only explicitly documented exception boundary.
- [ ] Confirm logs, exports, and errors do not expose passwords, tokens, API keys, or sensitive environment values.
- [ ] Confirm every generated-file write path uses the intended lock, backup, syntax-check, and atomic-replacement boundary.

A checked CI item means the workflow passed. It does not replace manual machine acceptance.
