# Task 21-2 Windows PowerShell 7 Acceptance

Run independently under `pwsh`. Do not reuse PowerShell 5.1 results. Record exact PS7 version, OS/architecture, policy/language mode, artifact SHA256, sanitized profile/config path, Actual, Result, Evidence, Reproduction notes, and redaction confirmation.

## PS7-001/PS7-002: clean start and lifecycle

1. Start the candidate under an isolated PS7 profile/config directory.
2. Add/edit/list/search/fuzzy/limit/tag/sync/reload/disable/rename/delete aliases.
3. Repeat in a new PS7 session and compare only sanitized state summaries.

Expected: independent persistence and reload behavior; no claim that a child process updates a parent session.

## PS7-003: profile and OneDrive

1. Record `$PROFILE.CurrentUserAllHosts` and the profile source without exposing usernames.
2. Test explicit profile override.
3. Repeat under OneDrive Documents if applicable.

Expected: PS7 path resolution is independent of PS5.1; override and isolation work; network/redirected path limitations are reported.

## PS7-004: encoding and line endings

1. Prepare disposable profile variants with BOM/no-BOM and CRLF/LF.
2. Install/remove loader and inspect byte summaries.
3. Inspect generated PS7 file encoding separately from PS5.1.

Expected: profile bytes are preserved; PS7 generated encoding follows the documented no-BOM policy.

## PS7-005: preemption/reserved names

1. Test `ls`, `cp`, and `gc` with disposable definitions.
2. Verify `Get-Command` returns the expected managed function only when the definition is safely preemptible.
3. Test ReadOnly/Constant names and record rejection.

Expected: removable aliases/functions are cleared; reserved names are rejected; user definitions are not silently deleted.

## PS7-006: policy/language guidance

1. Record policy scopes and `$ExecutionContext.SessionState.LanguageMode`.
2. Test Restricted, AllSigned, Group Policy, and ConstrainedLanguage only where an isolated policy context is available.
3. Do not use bypass switches or change policy automatically.

Expected: accurate guidance and explicit limitation/block status.

## PS7-007: native argv

1. Run the exact argument matrix against the dumper.
2. Test PS7 versions below and above the native argument passing behavior boundary if installed.
3. Record exact argv and expected limitations separately from PS5.1.

## PS7-008: ACL and target protection

1. Test safe and unsafe disposable target locations.
2. Confirm warning/block behavior.
3. Purge and verify referenced target bytes remain unchanged.

## PS7-009: loader/uninstall

1. Install marked loader twice.
2. Remove loader in retain and purge trials.
3. Verify unrelated profile content, generated files, and target protection.

Expected: idempotent marked changes; no unmanaged deletion.
