# Uninstall

`aliasmgr uninstall` removes Alias Manager's marked loader block. Retain mode keeps generated definitions and referenced target files so the generated script can continue to run without Alias Manager. Purge mode additionally removes Alias Manager generated files, database, and journal files under the configured directory.

The uninstall process never deletes, moves, or edits referenced EXE, BAT, CMD, Python, PowerShell, Shell, or JAR targets. In non-interactive environments destructive operations require explicit confirmation.

If the application is removed manually, loader guards (`[ -r ... ]` or `Test-Path`) prevent Shell startup errors when generated files are absent.

## Package-manager boundary

Package-manager `postrm`/uninstall hooks are non-interactive and must not guess whether aliases or generated files should be retained. They should direct the user to run `aliasmgr uninstall` first; no GUI prompt is guaranteed from a package hook. A future Windows MSI custom action must call the same core uninstall operation and present explicit retain/purge choices in the installer UI.

The current CLI/core implementation is the source of truth for retain and purge cleanup. Installer integration is not implemented in this pre-GUI phase.

## Verification boundary

CI verifies target-file protection and generated/database cleanup. Manual machine acceptance is still required for a user's real profile, symlinked RC/Profile files, package-manager hooks, and Windows installer behavior.
