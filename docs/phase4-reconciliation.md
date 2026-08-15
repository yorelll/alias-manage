# Phase 4 Tasks 1–17 Reconciliation

## Scope

This is the final non-GUI evidence reconciliation for Tasks 1–17. The broad Task 1–17 checkboxes in `plan.md` remain the only broad requirements. This document does not claim GUI implementation, GUI CI, package installation, real-user profile compatibility, or visual/manual acceptance.

## CI evidence

- Fast CI: run `31875233532` — Linux tests, Windows tests, and lint succeeded.
- Integration: run `31875307387` — Bash/Zsh, PowerShell 5.1/7, lock, CLI-layer isolation, and matrix jobs succeeded.
- Additional slice evidence remains attached to the relevant `plan.md` subordinate items, including storage, parser, generated-file integrity, recovery, detection, and tag-filter runs.

## Status categories

| Category | Meaning |
|---|---|
| Implemented and CI verified | Source and focused behavior tests exist, and the relevant CI workflow succeeded. |
| Implemented with evidence but environment-dependent | Code/tests exist, but real user profile, plugin, installer, or machine acceptance is still required. |
| Not implemented | The requirement has no sufficient implementation/evidence and remains unchecked. |
| GUI-dependent/out of scope | Deliberately excluded from this non-GUI gate. |

## Remaining non-GUI gaps

- Filesystem reliability detection and `UnreliableFilesystem` reporting.
- Windows ACL writable-path detection.
- Complete native command argument/version matrix.
- PowerShell profile/OneDrive resolution, BOM/CRLF preservation, ExecutionPolicy diagnostics, and real `Get-Command` session behavior.
- Fingerprint protection, override recovery, tombstone reload regression, and durable doctor decisions.
- SQLite transaction binding for synchronization, committed forward recovery, crash injection, and durable `shell_state` persistence.
- Full CLI reload guidance, list/find sorting and field/format/limit handling.
- Import confirmation persistence and complete TOML conflict/safety matrix.
- Real package-hook/MSI integration and clean-machine acceptance.

## Explicitly excluded

GUI implementation and GUI validation; visual acceptance; Tauri build jobs; frontend tests; real terminal experience; real user PowerShell profiles; oh-my-zsh/plugin ordering; symlink behavior on the user’s Windows installation; and installer/package-hook execution.

## Documentation slice 5 status

The pre-GUI documentation/release-boundary slice is implemented across `README.md`, `docs/cli-commands.md`, `docs/release-checklist.md`, `docs/architecture.md`, `docs/security.md`, `docs/testing.md`, `docs/limitations.md`, `docs/exit-codes.md`, `docs/ci-workflow.md`, and `docs/uninstall.md`. The remaining clean-machine and real-user acceptance items are intentionally unchecked.

## Security gate

The normal path continues to reject advanced shell mode and `RawShellCommand`, does not use `eval` or `Invoke-Expression` in ordinary generated paths, filters sensitive import environment keys, preserves structured argv boundaries, and protects referenced target files during uninstall.

This reconciliation is a status record, not a claim that the full GUI MVP or release checklist is complete.
EOF
)