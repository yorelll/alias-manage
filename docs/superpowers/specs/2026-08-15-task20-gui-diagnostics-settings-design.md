# Task 20 GUI Diagnostics, Transfer, and Settings Design

## Goal

Add core-backed Doctor, import/export, Settings, and uninstall panels to the Task 18/19 GUI without duplicating core business logic or silently performing destructive actions.

## Scope

Task 20 includes:

- Doctor findings grouped by severity with per-Shell details;
- read-only generated-code preview and reload-command copy;
- graded safe repair actions;
- import preview and explicit confirmation;
- read-only overridden-definition display and manual recovery copying;
- limited Settings editing with a single save operation;
- Retain/Purge uninstall selection, target protection report, and Purge second confirmation;
- core/command/frontend tests and existing Linux/Windows frontend CI.

Task 20 does not include GUI visual acceptance, real window startup acceptance, MSI/package-hook integration, complete Profile/RC editing, automatic ExecutionPolicy changes, automatic user-definition recovery, or unrelated Task 21 release packaging.

## Approved decisions

- **Data flow:** all panels use real core-backed commands.
- **Confirmation:** graded confirmation. Read-only actions execute directly; reversible writes show a confirmation summary; destructive actions require a second confirmation.
- **Doctor:** hybrid organization: severity summary at the top, expandable Bash/Zsh/PowerShell 5.1/7 details below.
- **Doctor actions:** safe/reversible generated-file refresh may be invoked; Profile/RC, user definitions, and ExecutionPolicy remain manual-only.
- **Overrides:** read-only display with copyable recovery text; no automatic execution or restore command.
- **Import:** single-page preview plus bottom confirmation; preview never writes, confirmation persists accepted records by reusing `AliasService::create/update`.
- **Settings:** limited editable settings with draft state and one unified save; configuration directory is read-only; no Profile or ExecutionPolicy mutation.
- **Uninstall:** Retain/Purge radio selection with dynamic risk summary; Purge requires a second confirmation and calls core uninstall; referenced target files remain protected.

## Architecture and commands

```text
DoctorPanel       ─┐
ImportPanel       ─┼─ invoke → thin Tauri commands → aliasmgr-core
SettingsPanel     ─┤
UninstallPanel    ─┘
```

Commands are DTO adapters only:

- `doctor_status`
- `generated_preview`
- `reload_command`
- `import_preview`
- `import_confirm`
- `config_get`
- `config_save`
- `uninstall_preview`
- `uninstall_confirm`
- `overridden_definitions`

Core owns detection, severity/status calculation, transfer safety, service persistence, configuration serialization, uninstall cleanup, and target protection. Tauri commands do not embed SQL, search rules, validation rules, or destructive policy.

## UI behavior

### DoctorPanel

The top summary counts normal, warning, failed, and manual-action findings. Shell sections expand to show loader, generated file, checksum, syntax, permission, policy, and stale-state details available from core. Actions include generated-file refresh where safe, copy reload command, generated-code preview, config-path opening, and Settings navigation. No automatic Profile/RC or ExecutionPolicy changes are offered.

### ImportPanel

The page displays selected file metadata, aliases, skipped/unsupported records, sensitive-variable filtering, relative-path warnings, conflict strategy, and potential overwrite items. The preview command is read-only. The bottom confirmation invokes persistence only after explicit user confirmation. Failure preserves the report and draft state.

### SettingsPanel

The page loads a draft from core. Editable settings are default Shell, backup retention, log retention, and relative-path enablement. Configuration directory is read-only. A single Save action validates and writes the draft; failure leaves the draft intact. Profile paths and ExecutionPolicy are displayed as boundaries, not edited.

### UninstallPanel

Retain mode explains that generated definitions and targets remain while the loader is removed. Purge mode lists generated/database/journal cleanup and explicitly lists referenced target files as protected. Purge requires a second confirmation. Both paths call core uninstall and display a structured result.

### Overridden definitions

Show name, Shell, capture time, original definition, and recoverable status. `recoverable = 1` provides a copy action; `recoverable = 0` shows “not automatically recoverable.” No automatic restore is performed.

## Error and security rules

- Import preview never writes.
- Purge never runs without second confirmation.
- Errors are structured and non-sensitive.
- No environment dumps, complete profiles, tokens, or secret values appear in UI errors or logs.
- ExecutionPolicy `Restricted` displays the user-run instruction only; no `-ExecutionPolicy Bypass` and no automatic policy mutation.
- Uninstall never deletes, moves, or edits referenced EXE/BAT/CMD/Python/PowerShell/Shell/JAR targets.

## Testing and CI

Core/command tests cover severity grouping, read-only preview, reload command generation, import preview/confirm separation, per-record service persistence, config draft/save behavior, uninstall preview/confirmation, target protection, overridden-definition read-only behavior, and Restricted guidance.

Frontend tests cover summary/detail rendering, code preview, reload copy, import confirmation gating and failure retention, Settings draft/save failure behavior, Retain/Purge risk display and second confirmation, override copying, and Restricted guidance. Existing Linux/Windows GUI frontend jobs run npm test/typecheck/build. Fast CI runs core/workspace tests; integration runs transfer/uninstall/lock/isolation coverage.

## Manual boundary

Manual GUI validation remains responsible for real window startup, visual/interaction quality, actual file pickers, real Profile/RC behavior, enterprise PowerShell policy, package hooks/MSI, and clean-machine installation. Evidence is recorded through `docs/gui/` and is never inferred from frontend CI success.
