# Pre-GUI CLI MVP Completion Design

**Goal:** Complete the feasible non-GUI portions of Tasks 1–16 so Alias Manager is usable as a Linux/Windows CLI MVP before Tauri GUI work begins.

## Scope

The pre-GUI MVP includes core SQLite configuration, validation, search, structured execution, Bash/Zsh/PowerShell generation, loader management, synchronization, diagnostics, CLI CRUD, import/export, and uninstall. GUI, installers, packaging, visual interaction, and real-user profile acceptance remain outside this phase.

## Completion slices

1. **CLI lifecycle completeness:** finish update/rename, retired-name handling, delete/disable reload messaging, stable JSON/table output, and end-to-end tests in isolated configuration directories.
2. **Shell/sync integrity:** connect sync to database shell state and operation journal, add generated-file metadata/checksum, complete per-shell partial failure reporting, and add rollback/recovery tests.
3. **Shell integration verification:** run Bash/Zsh syntax and argument tests in Linux CI; run PowerShell 5.1/7 parser and argument tests in Windows CI; keep environment-dependent profile/symlink/oh-my-zsh checks explicit.
4. **Transfer/uninstall safety:** add TOML support, complete import safety reports and conflict handling, verify uninstall retain/purge modes never touch referenced targets.
5. **Documentation and release boundary:** update CLI usage, limitations, security, exit codes, CI capability boundaries, and identify remaining real-machine acceptance items.

## Design boundaries

The CLI delegates persistence, validation, execution, shell rendering, synchronization, transfer, and uninstall to `aliasmgr-core`. It does not access SQLite tables directly or construct shell code. Shell files remain derived artifacts; SQLite remains the source of truth. All writes use isolated temporary paths in CI and must preserve the no-eval/no-Invoke-Expression rule in normal execution.

## Error and safety behavior

Unsupported advanced shell records are rejected or reported as unsupported during import. Relative paths require an explicit base directory. Sensitive environment variables are omitted from exports. Existing user target files are read-only references during uninstall. Parent shells are never claimed to be updated by a child CLI process; reload commands are printed explicitly.

## Verification

Every implementation slice is committed and pushed on the feature branch. Rust, shell, and cross-platform checks run only in GitHub Actions. Local verification is limited to Git, GitHub CLI, Python, npm where applicable, and static inspection. GUI usability and real-user profile behavior remain manual acceptance items after the CLI MVP.
