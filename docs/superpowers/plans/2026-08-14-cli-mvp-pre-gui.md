# Pre-GUI CLI MVP Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete the feasible non-GUI portions of Tasks 1–16 so Alias Manager is a usable, CI-verified Linux/Windows CLI MVP.

**Architecture:** CLI commands delegate persistence, validation, search, execution, shell rendering, sync, transfer, and uninstall to `aliasmgr-core`. SQLite remains the source of truth; generated shell files remain derived artifacts. Every slice is committed and pushed separately, with Rust and shell verification performed only in GitHub Actions.

**Tech Stack:** Rust stable, Cargo workspace, SQLite/rusqlite bundled, Serde, clap, Bash/Zsh/PowerShell, GitHub Actions, `gh.exe`.

---

## File map

- `crates/aliasmgr-cli/src/commands.rs`: lifecycle, sync, diagnostics, transfer, and uninstall orchestration through core APIs.
- `crates/aliasmgr-cli/src/main.rs`: parse commands, dispatch, output, and stable exit codes.
- `crates/aliasmgr-cli/src/output.rs`: stable table/JSON rendering.
- `crates/aliasmgr-cli/tests/*.rs`: isolated CLI end-to-end tests.
- `crates/aliasmgr-core/src/storage.rs`: durable alias/shell/journal state APIs.
- `crates/aliasmgr-core/src/sync.rs`: atomic shell synchronization and recovery.
- `crates/aliasmgr-core/src/shells/*.rs`: shell rendering and loader behavior.
- `crates/aliasmgr-core/src/transfer.rs`: safe JSON/TOML import/export reports.
- `crates/aliasmgr-core/src/uninstall.rs`: retain/purge cleanup without touching targets.
- `.github/workflows/ci.yml`: fast Linux/Windows build, lint, and unit tests.
- `.github/workflows/integration.yml`: slow real-shell, parser, lock, and isolation matrix.
- `plan.md`: checkboxes for Tasks 1–16, updated only after CI evidence.
- `docs/architecture.md`, `docs/security.md`, `docs/testing.md`, `docs/limitations.md`, `docs/exit-codes.md`: CLI MVP documentation.

## Verification boundary

Never run `cargo`, `rustc`, `rustup`, Bash, Zsh, PowerShell, or Tauri locally. Push each implementation commit and use `gh` first; if unavailable, use `"D:/Program Files/GitHub CLI/gh.exe"`. Mark checkboxes only after a successful workflow run. Local checks may use `git`, `python`, and `npm` when available.

---

### Task 1: Complete CLI lifecycle semantics

**Files:** `crates/aliasmgr-cli/src/commands.rs`, `crates/aliasmgr-cli/src/main.rs`, `crates/aliasmgr-cli/tests/crud.rs`, `plan.md`

- [x] Add failing isolated tests for `update`, rename, `retired_names`, delete/reload messaging, and `find`.
- [x] Implement core-delegating update and rename functions; rename must insert the old name into `retired_names` before changing the record.
- [x] Implement `find` through core search APIs and stable table/JSON output.
- [x] Ensure delete/disable output includes reload guidance and does not claim the parent Shell was changed.
- [x] Push the semantic commit `feat: complete cli alias lifecycle semantics`.
- [x] Run CI and inspect failures with `gh.exe --log-failed` until the workflow succeeds.
- [x] Mark corresponding Task 14 checkboxes in `plan.md` only after CI success.

### Task 2: Bind synchronization to durable state

**Files:** `crates/aliasmgr-core/src/storage.rs`, `crates/aliasmgr-core/src/sync.rs`, `crates/aliasmgr-core/src/model.rs`, `crates/aliasmgr-core/tests` or module tests, `plan.md`

- [x] Add failing tests for journal rows, per-shell `shell_state`, revision increment, partial Shell failure, and retired-name pruning.
- [x] Add durable journal/shell-state CRUD APIs and make `SyncCoordinator::apply` update them transactionally where possible.
- [x] Add generated metadata lines for revision, managed names, retired names, and content checksum; compute checksum excluding the checksum line.
- [x] Implement recovery from journal backups and cleanup of temporary files; do not claim full rollback until a test restores a prior file.
- [x] Push `feat: bind synchronization to durable shell state`.
- [x] Verify with GitHub Actions and mark Task 11 metadata/state/partial-failure checkboxes supported by evidence.

### Task 3: Add real Shell CI integration

**Files:** `.github/workflows/integration.yml`, `crates/aliasmgr-tests/tests/bash.rs`, `zsh.rs`, `powershell.rs`, fixtures, `plan.md`, `docs/limitations.md`

- [x] Add isolated Bash tests for argument forwarding, quote boundaries, preemption, loader syntax, and tombstone cleanup.
- [x] Add isolated Zsh tests and install Zsh explicitly on Ubuntu runner.
- [x] Add Windows PowerShell 5.1 and 7 adapter tests using separate workflow matrix entries.
- [x] Record expected-limitation boundaries for PS native argument reconstruction and `.bat` parsing in the adapter tests/docs.
- [x] Push `ci: verify real shell adapter matrix`.
- [x] Run integration workflow with `gh.exe`, watch it, and confirm all matrix jobs succeed.

### Task 4: Complete safe transfer and uninstall interfaces

**Files:** `crates/aliasmgr-core/src/transfer.rs`, `crates/aliasmgr-core/src/uninstall.rs`, `crates/aliasmgr-cli/src/commands.rs`, `crates/aliasmgr-cli/tests/transfer.rs`, `uninstall.rs`, `plan.md`

- [ ] Add failing tests for TOML export/import, overwrite/rename/ask conflicts, unsupported records, relative paths, and safety warnings.
- [ ] Implement TOML alongside JSON with the same top-level metadata and filtering semantics.
- [ ] Expose CLI `import`, `export`, and `uninstall` dispatch; require confirmation for destructive non-TTY operations.
- [ ] Add tests that referenced EXE/BAT/CMD/Python/PowerShell/Shell/JAR files remain untouched in both uninstall modes.
- [ ] Push `feat: complete cli transfer and uninstall interfaces`.
- [ ] Verify `cargo test -p aliasmgr-cli --test transfer` and `uninstall` in CI; mark Task 16/17 checkboxes supported by evidence.

### Task 5: Documentation and CLI release boundary

**Files:** `README.md`, `docs/architecture.md`, `docs/security.md`, `docs/testing.md`, `docs/limitations.md`, `docs/exit-codes.md`, `docs/uninstall.md`, `docs/ci-workflow.md`, `plan.md`

- [ ] Document all currently supported no-GUI CLI commands, isolated config behavior, reload limitation, backups, transfer safety, and uninstall modes.
- [ ] Document known environment-dependent acceptance items: real PowerShell profiles/ExecutionPolicy, symlink RCs, oh-my-zsh, true terminal behavior, and GUI interaction.
- [ ] Align exit-code documentation with implementation and explicitly note any unimplemented mappings.
- [ ] Add a release checklist distinguishing CI evidence from manual machine acceptance; do not claim the GUI MVP is complete.
- [ ] Push `docs: document pre-gui cli mvp boundary`.
- [ ] Run the full fast CI and integration workflow, then mark only documentation/CI-supported checkboxes in `plan.md`.

## Final review gate

Before declaring the pre-GUI CLI MVP complete: inspect all Task 1–17 checkboxes, confirm every checked item has a code/test/CI basis, search ordinary generated paths for `eval` and `Invoke-Expression`, verify no remote URL contains credentials, run both workflows through `gh`, and report GUI/manual acceptance gaps separately.
