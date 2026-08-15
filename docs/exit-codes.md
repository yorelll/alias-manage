# Exit codes

| Code | Meaning |
|---:|---|
| 0 | Success |
| 1 | Unclassified error |
| 2 | Usage error |
| 3 | Name or definition conflict |
| 4 | Not found |
| 5 | Validation failure |
| 6 | Missing or unsafe target |
| 7 | Permission problem |
| 8 | Lock timeout |
| 9 | Syntax check failure |
| 10 | Partial synchronization failure |
| 11 | Rollback failure |
| 12 | Unsupported environment |
| 13 | Storage error |
| 14 | Non-interactive confirmation required |

## Implementation mapping

Current `AliasError` mappings are:

- Code 1: configuration/unclassified `Config` errors.
- Code 3: `AliasConflict`, exact-name conflict, PowerShell case-fold conflict, and reserved names.
- Code 5: alias-name, argument-template, and advanced-mode validation failures.
- Code 6: missing targets and unsafe target paths.
- Code 7: permission errors.
- Code 8: lock timeout.
- Code 12: newer schema and unsupported/uninstalled Shell.
- Code 13: checksum mismatch, SQLite, serialization, and I/O errors.

Codes 2, 4, 9, 10, 11, and 14 remain reserved or require additional command-path integration. Meanings are stable after publication.

Exit code 14 is the documented non-interactive confirmation meaning; current CLI confirmation errors are represented through the configuration error path until the dedicated error variant is added.

The exit-code table describes the CLI contract, not GUI or installer behavior.
