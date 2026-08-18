# Task 21-2 Package and Install Lifecycle Acceptance

Run only when actual package/installer artifacts are supplied. Do not treat the unsigned source archive as an installer. Use disposable Linux/Windows environments and preserve before/after snapshots of unrelated configuration.

For each case record platform/architecture, package/artifact version and SHA256, installer type, sanitized config paths, Actual, Result, Evidence, reproduction notes, and redaction confirmation.

## PKG-001: clean package install

1. Verify the supplied package checksum against the release artifact.
2. Install in a disposable user/environment.
3. Snapshot unrelated profiles/config before and after.
4. Start the application and record version.

Expected: installation succeeds without modifying unrelated config; application starts and uses the intended config root.

If no package exists: Result `NOT-APPLICABLE` or `BLOCKED`, with the missing artifact recorded.

## PKG-002: upgrade and rollback

1. Install the previous candidate and create disposable aliases/configuration.
2. Upgrade to the candidate artifact.
3. Verify aliases, generated files, backups, schema, and settings.
4. Roll back if supported and repeat verification.

Expected: state survives upgrade; rollback restores documented state; referenced targets remain unchanged.

## PKG-003: postrm/MSI hooks

1. Run Linux postrm or Windows MSI uninstall in a disposable account.
2. Observe whether the hook is non-interactive and whether it only handles the documented cleanup boundary.
3. Verify no target file, unmanaged profile line, or unrelated config is deleted.

Expected: hooks never prompt in package-manager context, never delete referenced targets, and direct users to explicit Alias Manager cleanup when required.

## PKG-004: unmanaged content

1. Put sentinel content in unrelated profile files and unrelated config directories.
2. Install, upgrade, uninstall, and purge in separate trials.
3. Compare sanitized hashes and line counts.

Expected: unmanaged content remains byte-for-byte unchanged or any documented normalization is explicitly reported.
