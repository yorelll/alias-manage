# Pre-GUI CLI MVP Release Checklist

This checklist defines the non-GUI release boundary. It does not certify the Tauri GUI, installers, package hooks, visual behavior, or real-user profile compatibility.

## Task 21-1 automated evidence

- [x] Current Phase B workflows passed: Fast CI `32093176327`; Integration `32093828288`; Release dry-run `32093979406`.
- [x] Linux/Windows matrix includes Bash, Zsh, PowerShell 5.1/7, lock, CLI, benchmark, and GUI npm cache jobs.
- [x] Release dry-run reports packaging inputs and does not create a Release automatically.
- [x] Unsigned artifact checksums and sanitized manifest are available in `alias-manager-unsigned-source` from run `32093979406` with seven-day retention.
- [x] No signing certificates, credentials, or publication tokens are stored in the repository.

## Task 21-2 prerelease artifact gate

- [x] `prerelease.yml` added with `workflow_dispatch` only — no push, PR, or schedule trigger.
- [x] Required inputs: `source_ref`, `version`, `create_prerelease`, `confirmation` (must equal `CREATE-PRERELEASE`).
- [x] Automated gates: Fast CI (lint + Rust workspace + GUI frontend), Integration (Bash/Zsh/PS5.1/PS7/CLI scripts), Task 23 security/source scan.
- [x] Builds unsigned Linux and Windows CLI binaries. GUI/installer artifacts (MSI/AppImage/deb/EXE) are `environment-blocked` in manifest.
- [x] Produces `SHA256SUMS`, sanitized `manifest.json` (`signed:false`, `published:false`), and bundles Chinese task21-2 manuals and terminal scripts.
- [x] Prerelease creation requires all gates passed AND `create_prerelease == true` AND `confirmation == CREATE-PRERELEASE`.
- [x] `contents: write` scoped to creation job only; no codesign/signtool/gpg/notarize.
- [x] Source-contract tests for workflow added to `crates/aliasmgr-gui/ui/src/script-contracts.test.ts`.
- [ ] Prerelease workflow dry-run (with `create_prerelease=false`) — pending CI run.
- [ ] Manual CLI/Shell/GUI/package acceptance — pending user execution.

## CI evidence

- [x] Task-specific CI evidence is recorded beside the relevant Task 1–20 subordinate items in `plan.md`.
- [x] GUI frontend jobs use the committed lockfile and are separate from the non-GUI core gate.
- [x] Integration uses `fail-fast: false`, concurrency cancellation, timeouts, pinned actions, and isolated configuration directories.

## CLI/core scope

- [x] SQLite is the configuration source of truth; generated shell files are derived artifacts.
- [x] Structured argv execution is used in ordinary mode.
- [x] Advanced shell mode and `RawShellCommand` remain rejected.
- [x] Bash/Zsh and PowerShell adapter behavior has CI evidence.
- [x] Import/export safety filters sensitive environment keys and rejects unsupported records.
- [x] Uninstall protects referenced target files.
- [x] Existing parent Shell sessions are not claimed to update automatically; reload guidance is required.
- [x] Durable shell state and committed forward recovery have focused CI evidence; full transaction binding/crash injection remains open.

## Documentation boundary

- [x] README identifies the CI-only Rust verification boundary and non-GUI limitations.
- [x] Architecture, security, testing, limitations, exit-code, CI, and uninstall documents exist.
- [x] Exit-code meanings are documented as stable after publication; unused future codes remain reserved.
- [x] Environment-dependent limitations are documented: PowerShell profiles/ExecutionPolicy, symlinked RC/Profile files, oh-my-zsh ordering, real terminal behavior, and GUI interaction.
- [x] Package-manager and Windows MSI integration boundaries are documented in `docs/uninstall.md`.
- [x] Automated behavior inventory is documented in `docs/ci-test-inventory.md`.

## Manual acceptance still required

- [ ] Clean Linux user-directory install/create/reload/delete/upgrade/uninstall flow.
- [ ] Clean Windows profile and PowerShell 5.1/7 install/create/reload/delete/upgrade/uninstall flow.
- [ ] Real user profile backups, symlink behavior, OneDrive redirection, ExecutionPolicy, and plugin ordering.
- [ ] Package-manager hooks and Windows installer behavior.
- [ ] GUI startup, visual presentation, interaction, and cross-platform acceptance.

## Security review

- [x] Source scan excludes ordinary generated paths from `eval` and `Invoke-Expression`; `reload --print` is documented as an explicit command-output boundary.
- [x] Existing import filtering and test fixtures avoid passwords, tokens, API keys, and sensitive environment values; Phase B adds workflow scans for environment dumps.
- [ ] Confirm every generated-file write path uses the intended lock, backup, syntax-check, and atomic-replacement boundary; this remains a code-review/release-gate item.

## Automated/manual boundary

The following remain manual or environment-dependent and must not be checked from CI alone: real GUI windows, visual UX, clean-machine installation, Windows Profiles/OneDrive/ExecutionPolicy/ACL, oh-my-zsh user ordering, package hooks, MSI, signing, publishing, and release approval.

A checked CI item means the workflow passed. It does not replace manual machine acceptance.
