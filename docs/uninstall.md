# Uninstall

`aliasmgr uninstall` removes Alias Manager's marked loader block. Retain mode keeps generated definitions and referenced target files so the generated script can continue to run without Alias Manager. Purge mode additionally removes Alias Manager generated files, database, and journal files under the configured directory.

The uninstall process never deletes, moves, or edits referenced EXE, BAT, CMD, Python, PowerShell, Shell, or JAR targets. In non-interactive environments destructive operations require explicit confirmation.

If the application is removed manually, loader guards (`[ -r ... ]` or `Test-Path`) prevent Shell startup errors when generated files are absent.
