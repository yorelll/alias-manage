# Pre-GUI CLI MVP Release Checklist

This checklist defines the non-GUI release boundary. It does not certify the Tauri GUI, installers, package hooks, visual behavior, or real-user profile compatibility.

Status note (2026-11-01): local code and documentation review is recorded below. No CI run has been triggered for the documentation branch, and all CI/manual/package items remain open until a run or a human acceptance report exists.

## Task 21-1 automated evidence (historical)

- [x] Current Phase B workflows passed: Fast CI `32093176327`; Integration `32093828288`; Release dry-run `32093979406`.
- [x] Linux/Windows matrix includes Bash, Zsh, PowerShell 5.1/7, lock, CLI, benchmark, and GUI npm cache jobs.
- [x] Release dry-run reports packaging inputs and does not create a Release automatically.
- [x] Unsigned artifact checksums and sanitized manifest are available in `alias-manager-unsigned-source` from run `32093979406` with seven-day retention.
- [x] No signing certificates, credentials, or publication tokens are stored in the repository.

These run IDs are historical evidence for earlier commits. They are not evidence for the current documentation branch.

## Task 21-2 prerelease artifact gate

These items describe committed workflow code, not a successful run of that code.

- [x] `prerelease.yml` added with `workflow_dispatch` only — no push, PR, or schedule trigger.
- [x] Required inputs: `source_ref`, `version`, `create_prerelease`, `confirmation` (must equal `CREATE-PRERELEASE`).
- [x] Automated gates: Fast CI (lint + Rust workspace + GUI frontend), Integration (Bash/Zsh/PS5.1/PS7/CLI scripts), Task 23 security/source scan.
- [x] Builds unsigned Linux and Windows CLI binaries via separate `build_linux_cli` and `build_windows_cli` jobs.
- [x] Contains actual Tauri GUI build jobs (`build_linux_gui`, `build_windows_gui`). Each job installs native dependencies, runs `cargo tauri build`, and collects produced AppImage/deb/MSI/NSIS artifacts. If a build fails, the job records `gui_build_status: environment-blocked` in its manifest and uploads a diagnostic artifact — it does not fabricate a GUI binary or fail the pipeline silently.
- [x] Produces `SHA256SUMS`, sanitized `manifest.json` (`signed:false`, `published:false`), and bundles Chinese task21-2 manuals and terminal scripts.
- [x] Prerelease creation requires all gates passed AND `create_prerelease == true` AND `confirmation == CREATE-PRERELEASE`.
- [x] `contents: write` scoped to creation job only; no codesign/signtool/gpg/notarize.
- [x] Source-contract tests for workflow and GUI build jobs exist in `crates/aliasmgr-gui/ui/src/script-contracts.test.ts`.
- [ ] Prerelease workflow dry-run (with `create_prerelease=false`) — pending CI run.
- [ ] GUI build jobs verified green for this code — previous runs were `environment-blocked`; pending a new run.
- [ ] Manual CLI/Shell/GUI/package acceptance — pending user execution.

## CI evidence

- [x] Task-specific CI evidence is recorded beside the relevant Task 1–20 subordinate items in `plan.md` (historical).
- [x] GUI frontend jobs use the committed lockfile and are separate from the non-GUI core gate (workflow code).
- [x] Integration uses `fail-fast: false`, concurrency cancellation, timeouts, pinned actions, and isolated configuration directories (workflow code).

## CLI/core scope

- [x] SQLite is the configuration source of truth; generated shell files are derived artifacts.
- [x] Structured argv execution is used in ordinary mode.
- [x] Advanced shell mode and `RawShellCommand` remain rejected.
- [x] Bash/Zsh and PowerShell adapter behavior has historical CI evidence.
- [x] Import/export safety filters sensitive environment keys and rejects unsupported records.
- [x] Uninstall protects referenced target files.
- [x] Existing parent Shell sessions are not claimed to update automatically; reload guidance is required.
- [x] Revisions advance on update, rename, remove, and shell removal; `sync` uses `max(alias revisions, tombstone revisions, 1)`.
- [x] `update` changes only supplied fields; `import --yes` persists atomically; `uninstall` removes marked loader blocks; `reload` honors `--shell`; `--quiet` suppresses success stdout.
- [x] PowerShell 5.1 generated files default to UTF-8 BOM; PowerShell 7 defaults to no BOM; checksums ignore the BOM.
- [x] `sync` returns exit 10 (`PartialSync`) when any Shell fails, and a syntax failure preserves the previous generated file.

## Local code review — 2026-11-01

Evidence is local source inspection, focused tests, and CLI spot checks against `target/debug/aliasmgr.exe` with isolated configuration roots. It is not a CI result.

- [x] Source scan for `eval` / `Invoke-Expression` / `iex` over `crates/**/*.{rs,ts,tsx}`, excluding `target/` build artifacts, returned zero source hits.
- [x] Advanced-mode rejection: `validation.rs` rejects `advanced_shell_mode` and `RawShellCommand` with `AdvancedModeUnsupported` (exit 5), with a unit test.
- [x] Sensitive export filtering: a local export of an alias with `TOKEN=secret` and `MODE=test` omitted `TOKEN` and kept `MODE`; `transfer.rs` has matching tests.
- [x] Configuration root precedence: `config_root.rs` passes; a local run confirmed `--config-dir` overrides `ALIASMGR_CONFIG_DIR`, and an empty environment variable falls back to the platform default.
- [x] Primary generated-file write path (`sync.rs::apply_internal`): lock, backup, `syntax::check`, temporary file, atomic rename. A syntax failure leaves the previous file untouched (`syntax_failure_preserves_previous_generated_file_and_leaves_no_temp`).
- [x] Syntax-check boundary is documented as best effort: an interpreter absent from `PATH` makes the check a no-op (`syntax.rs`, limitation documented).
- [ ] Recovery path in `sync.rs::recover_pending_operations` re-renders a committed journal entry without `syntax::check`. It is currently unreachable from `apply_internal`; this remains open for future recovery work.
- [x] CLI contract spot checks: `update` preserves omitted fields and advances the revision; missing alias returns exit 4; invalid shell/env returns exit 1; `import` preview persists nothing and `--yes` persists atomically; `uninstall` retain keeps generated/db, purge removes them, and both preserve referenced target files; missing `--yes` returns exit 14.
- [ ] Full-workspace `cargo fmt --check` and `cargo clippy -- -D warnings` gate — owned by the CI workstream and still pending.

## Documentation boundary

- [x] README and `docs/testing.md` describe the local Rust/frontend preflight workflow and the CI cross-platform boundary.
- [x] Architecture, security, testing, limitations, exit-code, CLI, CI, and uninstall documents exist.
- [x] Exit-code documentation distinguishes reachable codes (4, 10, 14), core-level code 9, clap usage code 2, and reserved code 11.
- [x] Environment-dependent limitations are documented: PowerShell profiles/ExecutionPolicy, symlinked RC/Profile files, oh-my-zsh ordering, real terminal behavior, and GUI interaction.
- [x] Package-manager and Windows MSI integration boundaries are documented in `docs/uninstall.md`.
- [x] Automated behavior inventory is documented in `docs/ci-test-inventory.md`, including the GUI Rust crate and the new config-root/syntax/BOM coverage.

## Manual acceptance still required

- [ ] Clean Linux user-directory install/create/reload/delete/upgrade/uninstall flow.
- [ ] Clean Windows profile and PowerShell 5.1/7 install/create/reload/delete/upgrade/uninstall flow.
- [ ] Real user profile backups, symlink behavior, OneDrive redirection, ExecutionPolicy, and plugin ordering.
- [ ] Package-manager hooks and Windows installer behavior.
- [ ] GUI startup, visual presentation, interaction, and cross-platform acceptance.

## Security review

- [x] Source scan excludes ordinary generated paths from `eval` and `Invoke-Expression`; `reload --print` is documented as an explicit command-output boundary.
- [x] Existing import filtering and test fixtures avoid passwords, tokens, API keys, and sensitive environment values; Phase B adds workflow scans for environment dumps.
- [x] Primary generated-file write path reviewed locally on 2026-11-01: lock, backup, syntax check, temporary file, and atomic replacement are present.
- [ ] Recovery committed-journal re-render path does not run a syntax check; it is currently unreachable but remains an open review item.

## Automated/manual boundary

The following remain manual or environment-dependent and must not be checked from CI alone: real GUI windows, visual UX, clean-machine installation, Windows Profiles/OneDrive/ExecutionPolicy/ACL, oh-my-zsh user ordering, package hooks, MSI, signing, publishing, and release approval.

A checked CI item means the workflow passed. It does not replace manual machine acceptance. A checked local-review item means the source/test/spot-check evidence listed beside it exists; it is not a CI result.
