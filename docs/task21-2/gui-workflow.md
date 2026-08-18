# Task 21-2 GUI Workflow Acceptance

Run on Linux and Windows separately using the candidate GUI artifact. CI frontend tests do not substitute for real-window, visual, accessibility, or runtime persistence acceptance. Record OS/architecture, GUI/app version, artifact SHA256, sanitized config path, window/display environment, Actual, Result, Evidence, reproduction notes, and redaction confirmation.

## GUI-001/GUI-002: launch/status/navigation

1. Launch the artifact.
2. Verify the status drawer shows version, detected Shell, and config directory.
3. Navigate Aliases, Sync, Doctor, and Settings.

Expected: window launches; status values are accurate; navigation does not crash or lose draft state.

## GUI-003/GUI-004: table/search/facets

1. Create aliases with descriptions, target types, Shells, enabled/disabled states, and tags.
2. Verify table fields, long-description tooltip, stable tag chips, and state indicators.
3. Test text, fuzzy, field, limit, one-tag, multi-tag AND, and clear filters.

Expected: fields render correctly; tooltip contains full description; filters are core-backed and clearing restores all records.

## GUI-005/GUI-006: wizard and validation

1. Open Add and Edit wizard.
2. Test basic/advanced sections, structured args, middle `{{args}}`, cwd, environment, Shell, tags, preview, Save, Cancel.
3. Test invalid, reserved, exact-conflict, and PowerShell case-fold-conflict names.

Expected: invalid saves are rejected; draft remains; no secrets are rendered; Save persists only after confirmation.

## GUI-007: Doctor

1. Open Doctor with missing generated file, stale state, and safe/manual findings available.
2. Expand per-Shell details.
3. Use safe actions and copy manual reload guidance.

Expected: severity grouping and actions are accurate; manual boundaries are labeled; no automatic Profile/ExecutionPolicy bypass occurs.

## GUI-008: import

1. Select disposable JSON/TOML input.
2. Preview and verify no persistence occurred.
3. Confirm and verify accepted records persist.
4. Test malformed, unsupported, sensitive, relative-path, and conflict inputs.

Expected: warnings/skips/unsupported records are retained in the report; failed confirmation preserves the draft/report.

## GUI-009: settings

1. Edit backup/log retention, default Shell, config directory, and relative-path setting.
2. Cancel and verify draft reset.
3. Save and restart GUI.
4. Test a read-only or invalid config path only in a disposable location.

Expected: saved settings persist; failures preserve draft and explain the boundary; real user config is not modified without explicit selection.

## GUI-010: uninstall

1. Create disposable alias and referenced target.
2. Open retain and purge options.
3. Verify risk summary and second confirmation.
4. Execute one mode and inspect unrelated content/target bytes.

Expected: retain/purge semantics are explicit; second confirmation is required; target files and unmanaged content remain.

## GUI-011: overrides

1. Create a recoverable and non-recoverable override record through the approved flow.
2. Open override details and copy only sanitized metadata/recovery text.

Expected: recoverability is shown accurately; copying does not execute or expose private content.

## GUI-012: accessibility/visual layout

1. Navigate using keyboard only.
2. Verify focus order, visible focus, buttons, labels, and error announcements.
3. Resize to minimum and wide window sizes; inspect table overflow, modal focus, and tooltip placement.

Expected: no trapped focus, clipped critical actions, unreadable contrast, or layout corruption. Record screenshots with sanitized paths/content.
