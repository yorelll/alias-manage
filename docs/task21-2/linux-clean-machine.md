# Task 21-2 Linux Clean-Machine Acceptance

Run on a dedicated Linux user or disposable VM. Do not infer any result from CI. Record OS, architecture, application/artifact version, SHA256, sanitized config root, and Shell versions.

## Shared case record

For every case below, append:

- Actual:
- Result: PASS | FAIL | BLOCKED | EXPECTED-LIMITATION | NOT-APPLICABLE
- Evidence: sanitized log/screenshot path and short command-output summary
- Reproduction notes:
- Sensitive-data redaction confirmed: yes/no

Stop immediately for unexpected deletion, profile corruption, secret exposure, or data loss.

## L-001: clean install/start/version

- Platform/architecture: Linux / record architecture
- Shell/version: record default Shell
- Config/profile path: disposable path, sanitized
- Preconditions: clean user/config directory; preserve a before snapshot of unrelated files
- Steps:
  1. Install or unpack the candidate artifact without using the real user configuration.
  2. Start the CLI or GUI shell entry point and request the version.
  3. Record the application version and artifact SHA256.
- Expected: application starts; version is reported; no unrelated configuration changes occur.
- Actual:
- Result:
- Evidence:
- Reproduction notes:
- Sensitive-data redaction confirmed: yes/no

## L-002: add native alias lifecycle

- Preconditions: isolated config root; harmless executable such as `printf` or a disposable fixture
- Steps:
  1. Add a native alias with one fixed argument.
  2. List aliases and retrieve the alias by exact name.
  3. Restart the application and repeat list/get.
- Expected: record persists with exact target, Shell, enabled state, and fixed arguments.
- Actual:
- Result:
- Evidence:
- Reproduction notes:
- Sensitive-data redaction confirmed: yes/no

## L-003: Python/JAR/change-directory targets

- Preconditions: disposable Python script, harmless JAR or documented unavailable JDK, and disposable working directory
- Steps:
  1. Create one alias for a Python script, one for `java -jar`, and one ChangeDirectory alias.
  2. Validate and preview each generated definition.
  3. Execute only harmless targets; do not use private target contents.
- Expected: each target type validates and renders; ChangeDirectory changes only the intended current Shell directory.
- Actual:
- Result:
- Evidence:
- Reproduction notes:
- Sensitive-data redaction confirmed: yes/no

## L-004: exact argv boundaries

- Preconditions: argument-dumper fixture or equivalent harmless dumper
- Steps:
  1. Test empty string, spaces, single/double quotes, CJK, backslash, `*`, and `?` as separate arguments.
  2. Test fixed arguments plus user arguments.
  3. Compare the dumper’s sanitized argv list element-by-element.
- Expected: every boundary and empty element is retained; no shell expansion or concatenation occurs.
- Actual:
- Result:
- Evidence:
- Reproduction notes:
- Sensitive-data redaction confirmed: yes/no

## L-005: args placeholder matrix

- Steps:
  1. Test implicit trailing `{{args}}` behavior.
  2. Test `fixed`, `{{args}}`, `post` middle insertion.
  3. Test repeated placeholder, placeholder with `pass_args=false`, and escaped literal `{{{{args}}}}`.
- Expected: valid forms preserve order; invalid forms reject without saving a record; draft/config remains unchanged.
- Actual:
- Result:
- Evidence:
- Reproduction notes:
- Sensitive-data redaction confirmed: yes/no

## L-006: working directory/environment/tags

- Steps:
  1. Configure a disposable working directory and non-sensitive environment marker.
  2. Run a harmless target that reports only the marker name, not secrets.
  3. Add tags and inspect list/search results.
- Expected: working directory and environment are applied; sensitive values never appear in logs/export; tags persist.
- Actual:
- Result:
- Evidence:
- Reproduction notes:
- Sensitive-data redaction confirmed: yes/no

## L-007/L-008: search, fuzzy, limit, tags

- Steps:
  1. Create aliases with distinct names, descriptions, targets, and tags.
  2. Test exact, substring, fuzzy, field, sort, descending, and limit queries.
  3. Select one tag, multiple tags, then clear all tags.
- Expected: ordering follows the requested query; multiple tags use AND; clearing restores the unfiltered set.
- Actual:
- Result:
- Evidence:
- Reproduction notes:
- Sensitive-data redaction confirmed: yes/no

## L-009/L-010: Bash and Zsh loader/reload

- Preconditions: disposable `.bashrc` and `.zshrc`; install Zsh only if it is available and approved
- Steps:
  1. Install the marked loader twice.
  2. Sync generated definitions.
  3. Start isolated interactive Bash and Zsh sessions and explicitly source/reload.
  4. Verify the alias and then remove the loader.
- Expected: loader is appended once, generated aliases load, removal preserves unrelated lines, and a child process does not claim to mutate an already-open parent Shell.
- Actual:
- Result:
- Evidence:
- Reproduction notes:
- Sensitive-data redaction confirmed: yes/no

## L-011/L-012: login/non-interactive and oh-my-zsh

- Steps:
  1. Test interactive and non-interactive Bash/Zsh separately.
  2. Test login chain behavior and whether `.bash_profile`/`.zprofile` sources the RC file.
  3. If oh-my-zsh is installed, define a post-loader override and record its source/order.
- Expected: documented interactive limitation is observed; post-loader override is reported, not silently reordered.
- Actual:
- Result:
- Evidence:
- Reproduction notes:
- Sensitive-data redaction confirmed: yes/no

## L-013/L-014: symlink, line endings, manual edit

- Steps:
  1. Point a disposable RC symlink at a user-owned target and install the loader.
  2. Verify the symlink remains and line endings are unchanged.
  3. Manually edit a generated file, then request sync.
- Expected: trusted symlink target is edited in place; manual edit is backed up and requires an explicit decision; no silent overwrite.
- Actual:
- Result:
- Evidence:
- Reproduction notes:
- Sensitive-data redaction confirmed: yes/no

## L-015/L-016: tombstone and recovery

- Steps:
  1. Create and load an alias.
  2. Disable, rename, and delete it in separate trials; reload each session.
  3. Inject a harmless syntax/failure boundary and run recovery.
  4. Inspect per-Shell status without printing full generated files.
- Expected: managed residue is removed, user definitions are preserved, prepared state restores, and per-Shell success/failure is accurate.
- Actual:
- Result:
- Evidence:
- Reproduction notes:
- Sensitive-data redaction confirmed: yes/no

## L-017: JSON/TOML import

- Steps:
  1. Export a disposable record to JSON and TOML.
  2. Preview import and confirm no database write occurred.
  3. Confirm import and verify persistence.
  4. Test sensitive environment key, relative path, unsupported advanced record, and each conflict strategy.
- Expected: preview is read-only; confirmation persists only accepted records; warnings/skips/unsupported items are explicit.
- Actual:
- Result:
- Evidence:
- Reproduction notes:
- Sensitive-data redaction confirmed: yes/no

## L-018/L-019: uninstall and upgrade/rollback

- Steps:
  1. Create an alias referencing a disposable target and unrelated profile content.
  2. Test retain and purge modes separately.
  3. Apply the candidate upgrade and, if available, rollback to the prior artifact.
  4. Verify target files and unrelated content.
- Expected: retain/purge selection is honored; referenced targets and unmanaged content remain; config/generated state survives or restores as documented.
- Actual:
- Result:
- Evidence:
- Reproduction notes:
- Sensitive-data redaction confirmed: yes/no
