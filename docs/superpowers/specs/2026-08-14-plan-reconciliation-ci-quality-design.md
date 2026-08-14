# Task 1–17 Plan Reconciliation and CI Quality Design

## Goal

Make `plan.md` an auditable source of truth for Tasks 1–17 and raise CI evidence from compile/string assertions to layered behavior tests, while keeping GUI implementation and GUI validation out of this phase.

## Four-phase execution

### Phase 1: Reconcile plan.md

Preserve every existing Task 1–17 broad checkbox as the total requirement. Under each broad item, add subordinate checkboxes for:

- foundation/implementation
- unit tests
- CLI integration tests, where applicable
- Linux CI verification
- Windows CI verification
- real Shell or user-environment acceptance
- documentation/security evidence, where applicable

A broad checkbox remains unchecked until every required subordinate item is complete or an explicit non-applicable decision is documented. A subordinate item may be checked only when a code location, test, or workflow run provides evidence. Environment-dependent items remain unchecked and are labeled as manual acceptance rather than being inferred from Rust compilation.

### Phase 2: Layered CI verification

Keep `ci.yml` fast and deterministic:

- Linux lint and format
- Linux workspace/unit tests
- Windows workspace/unit tests
- isolated temporary configuration directories

Expand `integration.yml` into a fail-fast-disabled matrix:

- Linux Bash real execution and syntax checks
- Linux Zsh real execution and syntax checks
- Linux CLI lifecycle end-to-end tests
- Linux SQLite migration/journal/recovery tests
- Windows PowerShell 5.1 renderer/parser/argument tests
- Windows PowerShell 7 renderer/parser/argument tests
- Windows CLI lifecycle end-to-end tests
- import/export/uninstall safety tests
- isolation and secret-redaction assertions

Use fixtures that dump argv as JSON and compare exact argument boundaries. Do not print complete profiles, environment dumps, tokens, or sensitive values in logs.

### Phase 3: Implement and verify by dependency order

Work in these slices, creating a semantic commit and pushing after each major slice:

1. Task 4 storage semantics: conflict errors, migration recovery, durable shell/journal state.
2. Task 8 Bash/Zsh behavior: real execution, loader, syntax, symlink/line-ending testable boundaries.
3. Task 9 PowerShell behavior: Parser API, BOM/encoding, parameter matrix, profile override and diagnostics.
4. Task 10 generated-file integrity: checksum, managed/retired metadata, fingerprint protection, override records.
5. Task 11 synchronization: transaction binding, real rollback/forward recovery, partial-failure state persistence.
6. Tasks 13–17 CLI: lifecycle, diagnostics, transfer, uninstall and their end-to-end tests.

For each slice: write failing tests first, implement the minimum behavior, push, run the relevant workflow with `gh` (fallback to the absolute `gh.exe` path), inspect failure logs, and only then update both the slice plan and corresponding `plan.md` subordinate checkboxes.

### Phase 4: Final reconciliation

Run fast and integration workflows, inspect all Task 1–17 totals and subordinate items, and produce a status table with these states:

- implemented and CI verified
- implemented with unit/CLI evidence but missing real-environment evidence
- blocked by environment/manual acceptance
- not implemented

Do not mark GUI items complete. GUI testing remains in `docs/gui/` for a later phase and is explicitly excluded from the Task 1–17 non-GUI completion gate.

## Evidence contract

Every completed subordinate checkbox must point to at least one of:

- source file/function
- focused unit test
- CLI end-to-end test
- CI workflow job and run ID
- documented manual acceptance procedure

A compile-only workflow cannot close a behavior checkbox. A string-content assertion cannot close a real Shell execution checkbox. Manual acceptance items require a user-run report and remain separate from CI status.

## Safety and scope boundaries

The normal execution path must continue to reject advanced shell mode and RawShellCommand, avoid `eval` and `Invoke-Expression`, preserve structured argv boundaries, omit sensitive export/log values, and never delete referenced target files. No GUI code, packaging, installer, or visual acceptance work is included in this phase.
