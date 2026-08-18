# Task 21-2 Bash/Zsh User Configuration Acceptance

Use a disposable user configuration or a dedicated dotfiles checkout. This document focuses on real RC/Profile ordering and user-owned configuration boundaries that CI cannot prove.

For each case record Shell version, sanitized RC path, symlink target classification, artifact SHA256, Actual, Result, Evidence, reproduction notes, and redaction confirmation.

## U-001: marked loader append/idempotence

1. Snapshot unrelated RC lines using a sanitized hash and line count.
2. Install Bash and Zsh loaders separately, twice each.
3. Confirm exactly one complete marker pair at the end.

Expected: only one marked block exists; unrelated content and line endings remain unchanged.

## U-002: login and non-interactive chains

1. Start interactive and non-interactive Bash.
2. Start login Bash and inspect whether it sources `.bashrc` without uploading the file.
3. Repeat for Zsh and `.zprofile`/`.zshrc`.

Expected: interactive behavior matches documented limitation; missing login chain is reported with manual guidance and not auto-modified.

## U-003: oh-my-zsh/post-loader override

1. Use an isolated oh-my-zsh installation if available.
2. Define a disposable same-name alias in a plugin loaded after the Alias Manager block.
3. Run Doctor or inspect source order and record the overriding file/line only.

Expected: post-loader override is reported; Alias Manager does not reorder or silently overwrite plugin configuration.

## U-004: symlink RC and line endings

1. Point `.bashrc` or `.zshrc` at a trusted user-owned symlink target.
2. Install/remove the loader.
3. Verify symlink identity and target content hash.

Expected: trusted symlink remains; target is edited in place; unsafe cross-user/global-writable links are blocked or require explicit classification.

## U-005: manual generated edit/checksum

1. Sync a disposable generated file.
2. Make a harmless manual edit.
3. Sync again and observe backup and decision prompt/report.

Expected: backup exists; user edit is not silently discarded; decision and evidence are recorded.

## U-006: live tombstone/fingerprint cleanup

1. Create and load an alias in a single interactive session.
2. Delete/disable/rename it in the application.
3. Reload the generated file in the same session.
4. Recreate a same-name user definition and repeat reload.

Expected: managed definitions are removed; user-recreated definitions are preserved and reported as skipped; if the host cannot expose a reliable definition fingerprint, mark EXPECTED-LIMITATION or BLOCKED.

## U-007: override recovery

1. Define a disposable user alias/function.
2. Force an explicitly confirmed managed replacement.
3. Inspect only the sanitized override record and recoverability flag.
4. Test parseable and intentionally unparseable definitions separately.

Expected: parseable definition is recoverable; unparseable definition is marked non-recoverable; original text is never pasted into the report.
