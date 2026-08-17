# Pre-Task 21 Completion and Release Governance Design

## Goal

Complete every implementation-feasible unchecked item before Task 21, then execute Task 21 as two explicit tracks: automated CI/release verification and detailed human-assisted acceptance under Task 21-2.

## Completion policy

The original broad checkboxes remain the sole broad requirements. Subordinate items may be checked only with source, focused tests, CI run IDs, or a documented human report. GUI visual acceptance, real-user profiles, installers, and terminal behavior are never inferred from compilation.

Before Task 21 begins, all implementation-feasible items in Tasks 1–20 must be completed or explicitly reclassified as manual/environment-dependent. Post-MVP Tasks 24–26 remain out of scope and must not be pulled into Task 21.

## Phase A: Pre-Task21 implementation closure

### Core/storage/sync

- Filesystem reliability detection and `UnreliableFilesystem` reporting where platform APIs permit.
- Windows writable-path/ACL detection.
- Complete native argument/version matrix that can be verified in CI.
- `SortField` and descending search behavior.
- Durable doctor shell-state/checksum/permission findings.
- SQLite transaction binding for journal/shell state where feasible.
- Committed forward recovery and durable partial-failure state.
- Real tombstone cleanup, fingerprint protection, and override recovery tests.

### CLI/transfer/uninstall

- Per-Shell reload guidance for delete/disable/rename.
- Complete list/find fields, sorting, output format, and limit handling.
- Import confirmation persistence through the core service.
- Complete TOML conflict/safety matrix.
- Marked loader install/uninstall idempotence.
- Stable non-interactive and all published exit-code behavior.

### GUI runtime

- Core-backed Task 19/20 command persistence rather than placeholder responses.
- Runtime list/search/tag/create/update behavior.
- Doctor, import, settings, and uninstall command behavior against core.
- Frontend tests for every Task 19/20 feature and error/confirmation path.
- Standalone Tauri compile only if platform dependencies can be provisioned honestly in CI; otherwise record the exact blocker and keep real-window acceptance manual.

## Phase B: Task 21-1 automated verification and release

Task 21-1 expands CI without weakening the existing fast/integration gates:

- Linux Ubuntu 24.04 dependencies and Bash/Zsh real execution.
- Oh-my-zsh isolation job covering preemption and loader position.
- Windows PowerShell 5.1 and 7 separate jobs covering profile discovery, parser, BOM/CRLF, built-in alias preemption, and native argument matrix.
- Focused core/CLI/GUI tests for every feature and error branch.
- Isolated HOME/config/Profile assertions before and after tests.
- Secret/log redaction checks.
- 500/1000 alias load benchmark with recorded results.
- Release workflow on tag/manual dispatch only.
- Ubuntu 22.04 packaging compatibility job.
- Unsigned artifact verification; signing remains Secrets-only and is not automatically published.
- Full exit-code regression and security scan.

Task 21-1 must produce a status table with workflow run IDs, job names, test counts, failures, artifacts, and known manual gaps.

## Phase C: Task 21-2 human-assisted acceptance

Task 21-2 is a separate manual track. It must not be closed by CI. The user receives a test matrix where each case has:

- stable test ID;
- platform/OS/architecture;
- Shell and version;
- clean config/profile path;
- preconditions;
- exact numbered steps;
- expected result;
- actual result;
- PASS/FAIL/BLOCKED/EXPECTED-LIMITATION/NOT-APPLICABLE;
- screenshot or sanitized output evidence;
- reproduction notes;
- redaction confirmation.

Manual groups:

1. Linux clean install/lifecycle: install, startup, add, edit, list, search, tag AND, preview, sync, reload, disable, rename, delete, restart Shell, upgrade, retain uninstall, purge uninstall.
2. Windows PowerShell 5.1 lifecycle: the same cases plus Profile path, BOM/CRLF, `ls` preemption, ExecutionPolicy, OneDrive path, ACL and target protection.
3. Windows PowerShell 7 lifecycle: the same cases independently; never treat PS 5.1 results as PS 7 evidence.
4. Bash/Zsh real user configuration: `.bashrc`/`.zshrc`, login chain, non-interactive guard, oh-my-zsh ordering, symlink RC, line endings, manual generated-file edits.
5. GUI workflow: startup, sidebar, list, search, tags, wizard, validation, conflicts, preview, Doctor, Import, Settings, Retain/Purge, reload messaging, visual layout, keyboard/focus behavior.
6. Package/install lifecycle: package hook, MSI/install/uninstall, upgrade, rollback, unmanaged-content preservation.

Human feedback is returned as the completed matrix plus a concise blocker list. A failed or blocked case cannot be silently checked off; remediation creates a new implementation slice or records an explicit environment limitation.

## Phase D: Final release decision

Release requires:

- all feasible pre-Task21 items closed with evidence;
- Task 21-1 green across all required jobs;
- Task 21-2 manual report reviewed, with no unexplained failures;
- security scan clean or explicitly accepted;
- artifact checksums recorded;
- GUI/installer/manual limitations clearly separated from automated evidence;
- no automatic merge, tag, release, or publish without explicit user instruction.

The final report must distinguish: automated pass, human pass, environment blocked, expected limitation, not implemented, and post-MVP scope.
