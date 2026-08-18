# Task 21-2 Windows PowerShell 5.1 Acceptance

Run independently under Windows PowerShell 5.1, not `pwsh`. Use a disposable `USERPROFILE`, temporary `LOCALAPPDATA`/`APPDATA` where possible, and an explicit profile override. Do not reuse PowerShell 7 results.

For every case record OS/build, architecture, exact `$PSVersionTable.PSVersion`, execution-policy scopes, artifact SHA256, sanitized profile/config path, Actual, Result, Evidence, Reproduction notes, and Sensitive-data redaction confirmed.

## PS51-001: clean start/version

1. Install/unpack the candidate artifact in a disposable location.
2. Launch Windows PowerShell 5.1 with the isolated profile/configuration.
3. Record application and Shell versions.

Expected: starts without modifying unrelated profile content; version is correct.

## PS51-002: full lifecycle

1. Add native, Python, PowerShell-script, and ChangeDirectory aliases using disposable targets.
2. Edit target/args/description/tags.
3. List, search, fuzzy-search, sort, limit, filter tags, sync, reload, disable, rename, and delete.
4. Repeat after a new isolated PowerShell 5.1 session.

Expected: all state persists; reload removes only managed residue; errors preserve drafts/state.

## PS51-003: profile and OneDrive

1. Record `$PROFILE.CurrentUserAllHosts` without uploading the path.
2. Test a disposable profile override.
3. If Documents is OneDrive-redirected, repeat using the actual resolved path.

Expected: resolved profile is used, override takes precedence, and no real unrelated profile is modified.

## PS51-004: BOM/CRLF

1. Prepare a disposable profile with BOM and CRLF.
2. Install/remove the marked loader.
3. Inspect only byte-level encoding/line-ending summaries.

Expected: existing profile encoding/line endings are preserved; PS5 generated file follows the documented BOM policy.

## PS51-005: built-in aliases and reserved names

1. Test `ls`, `cp`, and `gc` preemption with disposable definitions.
2. Inspect `Get-Command` before/after reload.
3. Test aliases marked ReadOnly/Constant and record exact error category without dumping complete command metadata.

Expected: removable conflicts are preempted; reserved definitions are rejected as `NameReserved`; Constant definitions are never forcibly removed.

## PS51-006: ExecutionPolicy/language mode

1. Record effective policy by scope for the test user.
2. Test Restricted and AllSigned only in a disposable policy context approved for testing.
3. Record Group Policy or ConstrainedLanguage if present; do not bypass policy.

Expected: Doctor reports accurate guidance; no automatic policy change or `-ExecutionPolicy Bypass`; unsupported states are EXPECTED-LIMITATION or BLOCKED with reason.

## PS51-007: native argv

1. Run the argument dumper with empty, spaces, quotes, backslashes, CJK, wildcard, and trailing-backslash values.
2. Test middle `{{args}}` and fixed args.
3. Record exact argv boundaries and mark known PS5 host reconstruction limitations as EXPECTED-LIMITATION.

## PS51-008: ACL/target protection

1. Test a disposable safe target and a path with other-user write access if available.
2. Test unsafe target warning/block.
3. Run purge uninstall and confirm target bytes remain unchanged.

Expected: unsafe paths do not receive an unverified safety grant; targets remain intact.

## PS51-009: loader/uninstall

1. Install loader twice and compare sanitized profile hashes/marker counts.
2. Uninstall/retain/purge as separate trials.
3. Confirm unmanaged profile content and target files remain.

Expected: marked operations are idempotent and safe.

## PS51-010: MSI/package hooks

1. If an MSI/package artifact exists, install and uninstall in a disposable Windows account.
2. Record whether hooks are non-interactive and whether they call the documented cleanup boundary.
3. If no artifact exists, mark NOT-APPLICABLE or BLOCKED with the missing input.

Expected: no package hook silently edits unrelated configuration or deletes targets.
