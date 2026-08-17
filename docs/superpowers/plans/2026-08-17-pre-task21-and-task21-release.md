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

- [ ] **Step 1: Write failing sort tests**

Add aliases with different `updated_at`, `created_at`, `target_type`, and `enabled` values. Assert each `SortField` and `descending` order. Add a CLI parse test for repeated `--tag`, `--sort`, `--desc`, `--limit`, and `--format json`.

- [ ] **Step 2: Implement stable sort behavior**

After scoring/filtering, sort by the selected field, apply descending only to the selected field, then use stable name/updated-at tie breakers. Preserve query score ordering when a text query is present unless an explicit sort field was supplied.

- [ ] **Step 3: Implement CLI list/find request propagation**

Pass `fuzzy`, `limit`, `tags`, sort field, descending, and output format from `cli::Command` into `SearchQuery`. Render complete JSON DTO fields when `--format json`; keep table output stable.

- [ ] **Step 4: Verify and commit**

Push a semantic commit and verify fast CI plus integration CLI jobs. Record run IDs in the ledger before checking the subordinate items.

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

- [ ] **Step 1: Write failing platform tests**

On Unix, test a known temporary directory and an explicitly other-writable directory. On Windows, test the ACL helper against a temporary directory and assert the result is a structured `PermissionDenied`/unsafe-path result rather than a panic. Add a filesystem reliability result type that can represent `reliable`, `fallback-delete`, and `unknown`.

- [ ] **Step 2: Implement conservative filesystem detection**

Add a helper that identifies supported local filesystem behavior where platform APIs make it reliable. If detection cannot be made reliable, return `unknown` and use the existing safe journal-mode fallback; do not claim NFS/CIFS/WSL certainty. Add `UnreliableFilesystem` only if it is mapped consistently through CLI exit codes and doctor output.

- [ ] **Step 3: Implement Windows ACL writable-path check**

Use a Windows-only API boundary or a conservative documented fallback. The function must never grant safety based on an unverified ACL. Keep Linux permission logic unchanged.

- [ ] **Step 4: Verify Linux/Windows CI and commit**

Record platform-specific results separately. If a platform API cannot be safely verified on the runner, classify it as environment-blocked instead of checking it.

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

- [ ] **Step 1: Write exact-boundary argv tests**

Cover empty strings, spaces, quotes, backslashes, CJK, wildcard characters, fixed args, user args, middle `{{args}}`, working directory, environment values, Python, PowerShell, JAR, native executable, and Batch/CMD metacharacters. Assert argv element count and exact boundaries without printing secrets.

- [ ] **Step 2: Implement only missing target-specific behavior**

Keep structured arrays. Reject unsafe Batch/CMD characters as currently specified. Do not add string evaluation, `eval`, `Invoke-Expression`, or concatenated command lines.

- [ ] **Step 3: Verify separate PS 5.1/7 and Linux Shell matrix**

Run through integration workflow with separate jobs and record the exact matrix entries. Update limitations for unsupported host-specific native argument reconstruction.

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

- [ ] **Step 1: Write failing tombstone/fingerprint tests**

Test delete/disable/rename followed by sync/reload cleanup. Test a user-recreated same-name definition and assert cleanup skips it. Test override capture with `recoverable = 1` and unparseable definitions with `recoverable = 0`.

- [ ] **Step 2: Implement fingerprint metadata and skip reporting**

Store fingerprints in generated metadata and compare current definitions before cleanup. Add skipped names to a structured result; never delete a definition whose fingerprint does not match Alias Manager’s managed fingerprint.

- [ ] **Step 3: Persist override snapshots**

Call `record_override` before forced replacement, preserve original text when parseable, and set `recoverable = 0` when it cannot be safely recovered.

- [ ] **Step 4: Verify real Bash/Zsh/PowerShell integration**

Use isolated Shell processes and no complete profile output. Keep oh-my-zsh and user-profile ordering as separate Task 21-2 manual cases if the runner cannot safely reproduce them.

---

### Task A6: Complete sync durable state and forward recovery

**Files:**
- Modify: `crates/aliasmgr-core/src/storage.rs`
- Modify: `crates/aliasmgr-core/src/sync.rs`
- Test: `crates/aliasmgr-core/src/storage.rs`
- Test: `crates/aliasmgr-core/src/sync.rs`
- Test: `crates/aliasmgr-tests/tests/lock_process.rs`

- [ ] **Step 1: Write failing transaction/state tests**

Assert journal state, revision_from/to, backups_json, shell_state status, partial failure persistence, and prepared/committed recovery. Inject failure at each replace/state-write boundary through a test-only operation seam, not process-kill loops.

- [ ] **Step 2: Bind shell state and journal updates to core transaction boundaries**

Update SQLite state only after the generated file replacement succeeds. Keep per-Shell failures independent while recording a durable status for every attempted Shell.

- [ ] **Step 3: Implement committed forward recovery**

When the journal says the database reached revision_to, rebuild missing/old generated files from the database snapshot. When the database did not commit, restore backups and remove temp files.

- [ ] **Step 4: Verify and commit**

Run fast and integration workflow jobs that cover lock, recovery, and partial failure. Do not mark real crash injection complete unless an actual process interruption test exists.

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

- [ ] **Step 1: Write failing command tests**

Cover per-Shell reload messages after delete/disable/rename, durable doctor findings, import preview without writes, import confirmation persistence, TOML skip/overwrite/rename/ask and safety warnings, loader install/uninstall idempotence, and target-file protection.

- [ ] **Step 2: Implement request propagation and persistence**

Use the core service for confirmed imports. Ensure preview mode cannot write. Return structured per-Shell reload guidance and stable non-interactive behavior.

- [ ] **Step 3: Verify all CLI tests in fast/integration CI**

Record every command test target and job result in the closure ledger.

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

- [ ] **Step 1: Replace placeholder Task 20 command responses**

Implement actual `doctor_status`, `import_preview/import_confirm`, `config_get/config_save`, `uninstall_preview/uninstall_confirm`, and `overridden_definitions` against core. Do not return empty placeholder vectors or silently echo requests.

- [ ] **Step 2: Add exhaustive frontend tests**

Cover every Task 19/20 feature and error branch: table fields, truncation, search/fuzzy/limit, tag count/AND/clear, wizard basic/advanced/preview/save, invalid names, reserved names, exact/case-fold conflicts, Doctor severity/Shell details/actions, import report/confirmation/failure, Settings draft/save/failure, Retain/Purge/second confirmation, protected targets, overrides copy/recoverable state, and Restricted guidance.

- [ ] **Step 3: Verify Linux/Windows frontend CI**

Run npm test/typecheck/build on both runners. Keep standalone Tauri Rust compilation and real window startup separately classified if native dependencies are unavailable.

---

# Phase B — Task 21-1 automated verification and release

### Task B1: Expand the Linux matrix

**Files:** `.github/workflows/integration.yml`, `.github/workflows/ci.yml`, `docs/ci-workflow.md`

- [ ] Add pinned Ubuntu 24.04 dependency installation for WebKitGTK/GTK packages required by any Tauri compile job.
- [ ] Add real Bash execution, syntax, argv fixture, loader/tombstone/fingerprint, permission, and clean isolated HOME checks.
- [ ] Add isolated oh-my-zsh job or explicitly record runner/tool installation failure as environment-blocked.
- [ ] Add Linux benchmark job for 500 and 1000 aliases with recorded output and no sensitive logs.
- [ ] Verify `fail-fast: false`, timeout, concurrency, caching, and paths-ignore behavior.

### Task B2: Expand the Windows matrix

**Files:** `.github/workflows/integration.yml`, `.github/workflows/ci.yml`, `docs/ci-workflow.md`

- [ ] Keep PS 5.1 and PS 7 jobs separate.
- [ ] Test parser, built-in alias preemption, Profile discovery, BOM/CRLF, ExecutionPolicy reporting, ACL, argument fixture, and target protection independently in both versions where applicable.
- [ ] Assert isolated USERPROFILE/LOCALAPPDATA/APPDATA and injected profile paths.
- [ ] Record unavailable native features as environment-blocked, not passed.

### Task B3: Add exhaustive automated test matrix

**Files:** `.github/workflows/integration.yml`, `crates/aliasmgr-tests/tests/*.rs`, `crates/aliasmgr-cli/tests/*.rs`, GUI test files

- [ ] Build a test inventory with one stable ID per behavior and map each ID to one test target/job.
- [ ] Cover model, validation, storage, migration, checksum, permissions, locks, argv, Bash, Zsh, PS5, PS7, sync, recovery, CLI lifecycle, search, transfer, uninstall, GUI frontend, and command contracts.
- [ ] Add failure-path assertions for invalid names, target missing, conflicts, checksum mismatch, permission denied, lock timeout, malformed import, unsupported records, partial sync, and purge confirmation.
- [ ] Ensure test output never dumps profiles, environment variables, tokens, or secret values.

### Task B4: Complete release workflow safely

**Files:** `.github/workflows/release.yml`, `docs/ci-workflow.md`, `docs/release-checklist.md`

- [ ] Keep triggers limited to tag `v*` and manual workflow dispatch.
- [ ] Add Ubuntu 22.04 packaging compatibility job only when packaging files exist.
- [ ] Build unsigned artifacts first; do not create a Release automatically.
- [ ] Use GitHub Secrets only for future signing; never commit certificates.
- [ ] Generate artifact checksums and a sanitized manifest.
- [ ] Add a release dry-run/status job that reports missing packaging inputs rather than pretending to publish.

### Task B5: Security and release gates

**Files:** `docs/release-checklist.md`, `docs/security.md`, `docs/limitations.md`, `plan.md`

- [ ] Search ordinary generated paths for `eval` and `Invoke-Expression`; document `reload --print` as a command-output boundary only.
- [ ] Verify advanced mode/RawShellCommand rejection.
- [ ] Verify all generated-file writes use lock, backup, syntax-check, and atomic replacement boundaries.
- [ ] Verify sensitive import/log/error redaction.
- [ ] Verify remote URLs contain no credentials.
- [ ] Run full automated workflows and record run IDs, job names, test counts, failures, artifacts, and known manual gaps.

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
