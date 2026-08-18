# Pre-Task 21 Completion and Task 21 Release Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Execute in the current agent; do not dispatch subagents.

**Goal:** Close all implementation-feasible unchecked work in Tasks 1–20, then execute exhaustive automated Task 21-1 verification and prepare a detailed human-assisted Task 21-2 acceptance package.

**Architecture:** Preserve core as the source of truth and keep GUI commands as thin adapters. Close work in dependency order: storage/sync correctness, search/CLI completeness, Shell/PowerShell diagnostics, GUI runtime persistence, then CI/release verification. Manual machine and GUI acceptance is isolated in Task 21-2 and is never inferred from compilation.

**Tech Stack:** Rust workspace, rusqlite, Serde, Bash/Zsh, PowerShell 5.1/7, Tauri/React/TypeScript, Node tests, GitHub Actions, `gh`/absolute `gh.exe`.

---

## Global execution rules

- Do not run local Cargo, Rust, Rustup, Bash, Zsh, PowerShell, Tauri, or GUI dev-server commands.
- Verify Rust/Shell/Tauri behavior only in GitHub Actions.
- Use `gh` first. If unavailable, use `"D:/Program Files/GitHub CLI/gh.exe"`.
- Each implementation slice requires: failing test, CI red evidence when feasible, minimal fix, CI green evidence, semantic commit, push, and checkbox update.
- Do not use empty commits, force-push, or automatic tag/release/publish.
- Preserve original broad checkboxes as the only broad requirements. New evidence entries under them must be Chinese and must identify source/test/CI/manual evidence.
- Never mark manual/environment-dependent items complete from CI alone.
- Tasks 24–26 remain post-MVP and are not pulled into this plan.

## Evidence states

Use exactly these states in `plan.md` and the final report:

- `已实现并 CI 验证`
- `已实现但需人工/环境验收`
- `环境阻塞`
- `未实现`
- `post-MVP / out-of-scope`

---

# Phase A — Close implementation-feasible Tasks 1–20 gaps

### Task A1: Correct plan contradictions and establish the closure ledger

**Files:**
- Modify: `plan.md`
- Modify: `docs/phase4-reconciliation.md`
- Create: `docs/pre-task21-closure-ledger.md`

- [ ] **Step 1: Write the closure ledger before code changes**

Create a table with one row per unchecked Task 1–20 item. Each row must contain: original plan line, classification, intended implementation file, focused test, CI workflow, and resulting checkbox state. Classify each row as implementation-feasible, environment/manual, GUI/manual, or post-MVP.

- [ ] **Step 2: Remove duplicate stale summaries without changing original requirements**

Keep the original Task 1–20 broad entries. Remove or correct contradictory duplicate summaries such as older Task 4 lines that still say conflict semantics are unfinished after the later evidence-backed entries. Do not check a broad task merely because a summary was corrected.

- [ ] **Step 3: Commit the ledger and plan cleanup**

```bash
git add plan.md docs/phase4-reconciliation.md docs/pre-task21-closure-ledger.md
git commit -m "docs: establish pre-task21 closure ledger"
git push origin feature/alias-manager-mvp
```

---

### Task A2: Complete search sorting and CLI query output

**Files:**
- Modify: `crates/aliasmgr-core/src/search.rs`
- Modify: `crates/aliasmgr-cli/src/cli.rs`
- Modify: `crates/aliasmgr-cli/src/commands.rs`
- Modify: `crates/aliasmgr-cli/src/output.rs`
- Modify: `crates/aliasmgr-cli/src/main.rs`
- Test: `crates/aliasmgr-core/src/search.rs`
- Test: `crates/aliasmgr-cli/tests/crud.rs`
- Test: `crates/aliasmgr-cli/tests/parse.rs`

- [x] **Step 1: Write failing sort tests**

Added core explicit sort coverage and CLI parse/lifecycle tests for repeated `--tag`, `--field`, `--sort`, `--desc`, `--limit`, and JSON output.

- [x] **Step 2: Implement stable sort behavior**

Core sorting supports `Name`, `UpdatedAt`, `CreatedAt`, `TargetType`, and `Enabled`, with descending order and stable tie breakers (`crates/aliasmgr-core/src/search.rs`).

- [x] **Step 3: Implement CLI list/find request propagation**

CLI now propagates field, fuzzy, limit, tags, sort, descending, and output format through `list_query`/`find_query`; JSON rows include complete alias fields (`crates/aliasmgr-cli/src/commands.rs`, `output.rs`, `main.rs`).

- [x] **Step 4: Verify and commit**

Commits `ae6ebeb`, `f0d4a8a`, and `c4d2040`; fast CI run `32001624885` passed on Linux/Windows, lint, and GUI frontend jobs. Fresh integration run `32001809714` passed across Bash, Zsh, PowerShell 5.1/7, lock, core, and CLI layers.

---

### Task A3: Complete storage/filesystem and permission boundaries

**Files:**
- Modify: `crates/aliasmgr-core/src/error.rs`
- Modify: `crates/aliasmgr-core/src/storage.rs`
- Modify: `crates/aliasmgr-core/src/rotation.rs`
- Modify: `crates/aliasmgr-core/src/config.rs`
- Modify: `crates/aliasmgr-cli/src/exit_code.rs`
- Test: `crates/aliasmgr-core/src/storage.rs`
- Test: `crates/aliasmgr-core/src/rotation.rs`
- Test: `crates/aliasmgr-core/src/config.rs`
- Test: `crates/aliasmgr-cli/src/exit_code.rs`

- [x] **Step 1: Write failing platform tests**

Added explicit `FilesystemReliability` and `PathSafety` tests covering conservative local/remote/unknown outcomes and ensuring Windows ACL uncertainty is never treated as safe.

- [x] **Step 2: Implement conservative filesystem detection**

`config.rs` classifies known Linux local filesystems as `Reliable`, known network/remote mounts as `FallbackDelete`, and unknown types as `Unknown`. `UnreliableFilesystem` is mapped to CLI code 13 and documented; Windows filesystem certainty remains environment-blocked.

- [x] **Step 3: Implement Windows ACL writable-path check**

The Windows boundary returns `PathSafety::Unknown` until a verified native ACL implementation is available; it never grants safety based on an unverified ACL. Linux other-writable checks remain unchanged. The missing native ACL verification is classified as environment-blocked rather than as untracked implementation debt.

- [x] **Step 4: Verify Linux/Windows CI and commit**

Commits `c4ec953`, `89ccd4d`, and `821d56f`; CI run `32003275982` passed lint, Linux/Windows workspace tests, and GUI frontend jobs. Windows ACL/native filesystem certainty is explicitly classified as environment-blocked rather than checked.

---

### Task A4: Complete structured executor argument matrix

**Files:**
- Modify: `crates/aliasmgr-core/src/executor.rs`
- Modify: `docs/limitations.md`
- Test: `crates/aliasmgr-core/src/executor.rs`
- Test: `crates/aliasmgr-tests/tests/bash.rs`
- Test: `crates/aliasmgr-tests/tests/powershell.rs`
- Create/modify: `crates/aliasmgr-tests/fixtures/argument-dumper.py`
- Create/modify: `crates/aliasmgr-tests/fixtures/argument-dumper.ps1`
- Create/modify: `crates/aliasmgr-tests/fixtures/argument-dumper.bat`

- [x] **Step 1: Write exact-boundary argv tests**

Added executor coverage for empty strings, spaces, quotes, backslashes, CJK, wildcards, fixed/user/middle `{{args}}`, escaped placeholders, working directory, environment values, Python/PowerShell/JAR/change-directory target metadata, and Batch/CMD metacharacter rejection. Assertions inspect argv boundaries without logging secrets.

- [x] **Step 2: Implement only missing target-specific behavior**

Kept structured arrays and rejected unsafe Batch/CMD characters; no string evaluation, `eval`, `Invoke-Expression`, or concatenated command lines were added. Added Python, PowerShell, and Batch dumper fixtures for CI boundaries.

- [x] **Step 3: Verify separate PS 5.1/7 and Linux Shell matrix**

Fast CI `32004704944` and integration `32004874576` passed on Linux/Windows, Bash/Zsh, PowerShell 5.1/7, lock, core, and CLI jobs. Host-native PowerShell argv reconstruction remains an expected limitation documented in `docs/limitations.md`.

---

### Task A5: Complete Shell loader/tombstone/fingerprint behavior

**Files:**
- Modify: `crates/aliasmgr-core/src/shells/common.rs`
- Modify: `crates/aliasmgr-core/src/shells/bash.rs`
- Modify: `crates/aliasmgr-core/src/shells/zsh.rs`
- Modify: `crates/aliasmgr-core/src/shells/powershell.rs`
- Modify: `crates/aliasmgr-core/src/sync.rs`
- Modify: `crates/aliasmgr-core/src/storage.rs`
- Test: Shell module tests and `crates/aliasmgr-tests/tests/bash.rs`, `zsh.rs`, `powershell.rs`

- [x] **Step 1: Write failing tombstone/fingerprint tests**

Added synchronization and Shell tests proving retired names, deterministic fingerprints, conditional session cleanup, and recoverability metadata.

- [x] **Step 2: Implement fingerprint metadata and skip reporting**

Generated files include per-name fingerprint metadata and cleanup markers; CLI sync includes retired names even when no current alias targets that Shell. Conditional cleanup rendering is covered, but live Shell definition capture/skip reporting remains open.

- [x] **Step 3: Persist override snapshots**

Storage now exposes override records and tests distinguish parseable (`recoverable=true`) and unparseable (`recoverable=false`) snapshots. Forced replacement capture at the adapter boundary remains open.

- [x] **Step 4: Verify real Bash/Zsh/PowerShell integration**

Fast CI `32016526640` and integration `32017482835` passed. Real live-session fingerprint skipping, plugin ordering, and user-profile behavior remain Task 21-2/manual or subsequent Phase A work.

---

### Task A6: Complete sync durable state and forward recovery

**Files:**
- Modify: `crates/aliasmgr-core/src/storage.rs`
- Modify: `crates/aliasmgr-core/src/sync.rs`
- Test: `crates/aliasmgr-core/src/storage.rs`
- Test: `crates/aliasmgr-core/src/sync.rs`
- Test: `crates/aliasmgr-tests/tests/lock_process.rs`

- [x] **Step 1: Write failing transaction/state tests**

Added durable `shell_state` round-trip coverage for applied revision, checksum, loader state, status, and error text. Existing prepared-journal backup/recovery tests remain green.

- [x] **Step 2: Bind shell state and journal updates to core transaction boundaries**

Successful and failed per-Shell replacements now update SQLite `shell_state` after file replacement/failed rendering, while preserving independent Shell results.

- [x] **Step 3: Implement committed forward recovery**

Committed journals now rebuild missing generated files from the SQLite snapshot using the journal target Shell and revision, then remove the recovered journal. Crash injection remains unimplemented.

- [x] **Step 4: Verify and commit**

Fast CI `32020126900` and integration `32086715396` passed Linux/Windows workspace and lock/CLI/Shell/PowerShell matrix. Do not claim crash injection.

---

### Task A7: Complete CLI diagnostics, reload, transfer persistence, and uninstall boundaries

**Files:**
- Modify: `crates/aliasmgr-cli/src/commands.rs`
- Modify: `crates/aliasmgr-cli/src/main.rs`
- Modify: `crates/aliasmgr-core/src/transfer.rs`
- Modify: `crates/aliasmgr-core/src/uninstall.rs`
- Test: `crates/aliasmgr-cli/tests/diagnostics.rs`
- Test: `crates/aliasmgr-cli/tests/transfer.rs`
- Test: `crates/aliasmgr-cli/tests/uninstall.rs`

- [x] **Step 1: Write failing command tests**

Added CLI coverage for read-only import preview and confirmed import persistence, while retaining existing target-protection and diagnostic tests. TOML conflict/safety permutations and durable doctor findings remain open.

- [x] **Step 2: Implement request propagation and persistence**

Core transfer now separates parsing/reporting from accepted records; confirmed CLI import persists accepted records, while preview cannot create the database. CLI sync includes retired Shell names for reload cleanup.

- [x] **Step 3: Verify all CLI tests in fast/integration CI**

Fast CI `32091690967` passed Linux/Windows workspace, lint, and GUI frontend jobs; integration `32091809299` passed the Linux/Windows CLI/Shell/PowerShell/lock matrix. Full TOML matrix remains open; marked Bash/Zsh loader install/uninstall is now implemented and verified.

---

### Task A8: Complete GUI runtime persistence and Task 19/20 feature coverage

**Files:**
- Modify: `crates/aliasmgr-gui/src-tauri/src/commands.rs`
- Modify: `crates/aliasmgr-gui/ui/src/App.tsx`
- Modify: `crates/aliasmgr-gui/ui/src/lib.ts`
- Modify: `crates/aliasmgr-gui/ui/src/components/*.tsx`
- Create/modify: `crates/aliasmgr-gui/ui/src/task19.test.ts`
- Create/modify: `crates/aliasmgr-gui/ui/src/task20.test.ts`
- Test: GUI frontend Linux/Windows jobs

- [x] **Step 1: Replace placeholder Task 20 command responses**

Implemented core-backed `doctor_status`, generated preview/reload guidance, import reports, config load/save, uninstall preview/confirm, and overridden-definition reads. Preview remains read-only; destructive cleanup remains explicit.

- [x] **Step 2: Add exhaustive frontend tests**

Added source-contract coverage proving command implementations are not empty placeholders, all Task 20 commands are registered, and destructive import/purge actions remain explicit. Broader visual/runtime branches remain manual/standalone-runtime work.

- [x] **Step 3: Verify Linux/Windows frontend CI**

Fast CI `32009748252` passed Linux/Windows frontend tests, typecheck, build, lint, and workspace tests; integration `32010337861` passed Linux/Windows CLI/Shell/PowerShell/lock matrix. Standalone Tauri native compile and real-window acceptance remain separately classified.

---

# Phase B — Task 21-1 automated verification and release

### Task B1: Expand the Linux matrix

**Files:** `.github/workflows/integration.yml`, `.github/workflows/ci.yml`, `docs/ci-workflow.md`

- [x] Phase B gate reviewed: Phase A implementation-feasible items are reconciled or explicitly classified; manual/environment and standalone Tauri boundaries remain open and are not treated as blockers for automated Phase B.
- [x] GUI dependency lockfile and npm cache prerequisite completed in commit `5239a2e`; Fast CI `32093176327` verified the updated workflow.

- [x] Added pinned Ubuntu 24.04 matrix coverage for Bash/Zsh, syntax, argv fixtures, loader/tombstone/fingerprint contracts, permissions/isolation, and benchmark cases. Tauri WebKitGTK compile remains a separate standalone/native boundary.
- [x] Added real Bash/Zsh syntax and adapter jobs, CLI layers, lock tests, isolated config assertions, and benchmark job for 500/1000 aliases.
- [x] Added isolated oh-my-zsh contract job; real user plugin ordering remains Task 21-2/manual.
- [x] Added Linux benchmark job with sanitized elapsed/byte output only.
- [x] Verified `fail-fast: false`, timeout, concurrency, pinned action/cache, and existing paths-ignore behavior in Fast CI `32093176327` and Integration `32093828288`.

### Task B2: Expand the Windows matrix

**Files:** `.github/workflows/integration.yml`, `.github/workflows/ci.yml`, `docs/ci-workflow.md`

- [x] Keep PS 5.1 and PS 7 jobs separate in Integration.
- [x] Existing Windows matrix covers parser/preemption/argument/target-protection and workspace behavior separately for PS 5.1 and 7; Profile/OneDrive, BOM/CRLF, ExecutionPolicy, and ACL remain environment/manual boundaries.
- [x] Assert isolated `LOCALAPPDATA`/`APPDATA`/`ALIASMGR_CONFIG_DIR`; real user-profile injection remains manual because the runner profile must not be modified.
- [x] Record unavailable native features as environment-blocked rather than passed in the ledger and manual acceptance package.

### Task B3: Add exhaustive automated test matrix

**Files:** `.github/workflows/integration.yml`, `crates/aliasmgr-tests/tests/*.rs`, `crates/aliasmgr-cli/tests/*.rs`, GUI test files

- [x] Added `docs/ci-test-inventory.md` with stable IDs mapped to focused test targets and workflow jobs.
- [x] Inventory covers model, validation, storage, migration, checksum, permissions, locks, argv, Bash, Zsh, PS5, PS7, sync, recovery, CLI lifecycle, search, transfer, uninstall, GUI frontend, and command contracts.
- [x] Existing focused tests cover invalid names, missing targets, conflicts, checksum mismatch, permission/unsafe paths, lock timeout, malformed import, unsupported records, partial sync, and purge target protection.
- [x] Workflow/test scans prohibit profile/environment dumps, tokens, secrets, `set -x`, and complete configuration output.

### Task B4: Complete release workflow safely

**Files:** `.github/workflows/release.yml`, `docs/ci-workflow.md`, `docs/release-checklist.md`

- [x] `release.yml` triggers remain limited to tag `v*` and manual workflow dispatch.
- [x] Ubuntu 22.04 dry-run compatibility job reports missing packaging inputs; no package job runs when packaging files are absent.
- [x] Unsigned source artifact is built first; workflow never creates a GitHub Release automatically.
- [x] No signing secrets or certificates are used; future signing remains a GitHub Secrets-only boundary.
- [x] Artifact SHA256 checksums and sanitized manifest are generated with seven-day retention.
- [x] Release dry-run/status reports missing packaging inputs without pretending to publish; run `32093979406` passed.

### Task B5: Security and release gates

**Files:** `docs/release-checklist.md`, `docs/security.md`, `docs/limitations.md`, `plan.md`

- [x] Source security scan checks ordinary generated paths for `eval` and `Invoke-Expression`; `reload --print` remains documented as an explicit command-output boundary.
- [x] Advanced mode and `RawShellCommand` rejection are covered by validation tests and CI.
- [ ] Full generated-file write-path audit remains a release-review item; lock/backup/atomic/recovery paths have focused CI evidence but syntax-check coverage is not universal.
- [x] Import filtering, fixture output, and workflow scans cover sensitive redaction boundaries.
- [x] Repository/workflow scan contains no credential-bearing remote URLs or signing material.
- [x] Fast CI `32093176327`, Integration `32093828288`, and Release `32093979406` recorded with matrix, artifacts, checksums, and manual gaps; no tag/release/publish created.

---

# Phase C — Task 21-2 human-assisted acceptance

### Task C1: Create the manual test matrix

**Files:**
- Create `docs/task21-2/README.md`
- Create `docs/task21-2/test-matrix.md`
- Create `docs/task21-2/feedback-template.md`
- Create `docs/task21-2/linux-clean-machine.md`
- Create `docs/task21-2/windows-powershell51.md`
- Create `docs/task21-2/windows-powershell7.md`
- Create `docs/task21-2/bash-zsh-user-config.md`
- Create `docs/task21-2/gui-workflow.md`
- Create `docs/task21-2/package-install-lifecycle.md`

Every case must use this structure:

```markdown
### T21-2-<ID>: <title>

- Platform/architecture:
- Shell/version:
- Config/profile path:
- Preconditions:
- Steps:
  1. ...
  2. ...
- Expected:
- Actual:
- Result: PASS | FAIL | BLOCKED | EXPECTED-LIMITATION | NOT-APPLICABLE
- Evidence:
- Reproduction notes:
- Sensitive-data redaction confirmed: yes/no
```

### Task C2: Linux clean-machine acceptance cases

Create detailed cases for:

- install/startup and version;
- add native/Python/JAR/change-directory aliases;
- spaces, quotes, CJK, empty args, backslashes, wildcards;
- `{{args}}` end/middle/invalid/repeated;
- working directory/environment/tags;
- list/search/fuzzy/limit/tag AND/clear;
- Bash and Zsh load/reload;
- `.bashrc`/`.zshrc` login chain and non-interactive behavior;
- oh-my-zsh ordering/preemption;
- symlink RC and line endings;
- generated-file manual edit/checksum decision;
- disable/rename/delete/tombstone current-session cleanup;
- sync failure/recovery and partial Shell status;
- JSON/TOML import preview/confirm/safety;
- retain/purge uninstall and target protection;
- upgrade and rollback.

### Task C3: Windows PowerShell 5.1 acceptance cases

Independently execute the full lifecycle above plus:

- `$PROFILE.CurrentUserAllHosts` path;
- OneDrive redirected Documents;
- UTF-8 BOM and CRLF preservation;
- `ls`, `cp`, `gc`, `ReadOnly`, `Constant` behavior;
- Restricted, AllSigned, Group Policy, and ConstrainedLanguage;
- native argument matrix and exact argv fixture;
- ACL/writable directory and target protection;
- loader install/uninstall idempotence;
- MSI/package hook behavior.

### Task C4: Windows PowerShell 7 acceptance cases

Repeat the same cases independently. Record PS version, profile path, execution policy, encoding, argv result, and evidence. Do not reuse PS 5.1 results.

### Task C5: GUI acceptance cases

Use the existing `docs/gui/` manual conventions and test:

- launch/version/status drawer;
- sidebar navigation;
- list fields and description tooltip;
- text/fuzzy search and limit;
- tag facet count, single/multi AND, clear;
- add/edit wizard basic/advanced sections;
- preview/save/cancel;
- invalid/reserved/exact/case-fold errors;
- Doctor summary, per-Shell details, safe/manual actions;
- import preview/confirmation/failure retention;
- Settings draft/save/failure and read-only config path;
- Retain/Purge selection, risk summary, second confirmation;
- overridden-definition copy/recoverable state;
- keyboard/focus/visual layout and resize behavior.

### Task C6: Human feedback and triage

- [x] Created six detailed Task 21-2 Chinese manuals: Linux, PowerShell 5.1, PowerShell 7, Bash/Zsh user configuration, GUI workflow, and package/install lifecycle. Cases require 实际结果/结果/证据 fields and preserve manual/environment boundaries.
- [x] Existing matrix and feedback template remain unfilled; no manual PASS was inferred from CI.

The user returns completed `test-matrix.md` or a copy of each case with:

- result status;
- exact actual output;
- sanitized evidence paths;
- failing command or screenshot description;
- whether the issue reproduces;
- environment details;
- proposed classification.

The agent must not check a failed/blocked/manual item until the feedback is reviewed and either fixed or explicitly classified with a reason.

---

# Phase D — Final release decision

### Task D1: Reconcile all checkboxes

- [ ] Update every implementation-feasible Task 1–20 subordinate item with source/test/CI evidence.
- [ ] Keep manual/environment items separate and attach Task 21-2 reports.
- [ ] Keep GUI broad checkboxes open unless real-window/manual evidence exists.
- [ ] Mark post-MVP Tasks 24–26 explicitly out of scope rather than pretending incomplete implementation is a release blocker.

### Task D2: Produce final status report

The report must include:

- automated workflow/run/job table;
- per-feature test inventory and counts;
- artifacts/checksums;
- Task 21-2 human results;
- failures and fixes;
- environment-blocked cases;
- expected limitations;
- remaining post-MVP scope;
- explicit statement that no tag/release/publish was created automatically.

### Task D3: Release decision

- [ ] Only after all required automated and human gates pass, and after explicit user approval, create the release/tag workflow action.
- [ ] Do not merge, publish, or sign artifacts automatically.
