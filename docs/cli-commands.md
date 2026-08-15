# CLI Commands

This document describes the current pre-GUI CLI surface. It is a command contract, not a claim that all environment-dependent Shell behavior is accepted on every machine.

## Global options

- `--config-dir <PATH>`: explicit configuration root; takes precedence over `ALIASMGR_CONFIG_DIR`.
- `--format table|json`: output selection where the command currently supports it.
- `--no-color`, `--verbose`, `--quiet`: output controls.

CI and tests always use an isolated temporary configuration directory.

## Lifecycle

- `add <NAME> --exec <PROGRAM> [--arg VALUE]... [--shell SHELL] [--cwd PATH] [--env KEY=VALUE] [--tag TAG] [--no-pass-args]`
- `get <NAME>`
- `list [--sort FIELD] [--desc] [--limit N] [--tag TAG]...`
- `find <QUERY> [--fuzzy] [--limit N] [--tag TAG]...`
- `update <NAME>`
- `rename <OLD> <NEW>`
- `enable <NAME>` / `disable <NAME>`
- `remove <NAME> [--yes]`

Repeated tags use AND intersection through core `SearchQuery.tag_filter`. Existing commands still have incomplete full-field sorting/format and per-Shell reload output; those remain open in `plan.md`.

## Synchronization and diagnostics

- `sync [--dry-run]`: generate Shell files; dry-run must not create the database or write files.
- `reload --print`: print a single command for the user to source the generated file. A child process cannot update the already-open parent Shell.
- `doctor`: report currently implemented configuration/generated-file findings. Durable shell-state, policy, permission and complete checksum diagnostics remain incomplete.
- `shell detect`: report Shell detection.
- `shell install <SHELL>` / `shell uninstall <SHELL>`: command surface exists; complete marked-loader integration remains an open item.

## Transfer and uninstall

- `export <FILE>`: JSON by default, TOML for `.toml` paths.
- `import <FILE>`: preview/report path with format validation, sensitive environment filtering, unsupported-record and relative-path warnings. Persistence after explicit confirmation remains incomplete.
- `uninstall`: retain generated definitions and target references.
- `uninstall --purge-aliases`: remove Alias Manager generated/database files but never referenced target files.

Destructive non-interactive operations require `--yes` where supported. Package-manager hooks and MSI integration are documented separately in `docs/uninstall.md` and are not implemented in this pre-GUI phase.

## Related documentation

See `README.md`, `docs/security.md`, `docs/testing.md`, `docs/limitations.md`, `docs/exit-codes.md`, `docs/ci-workflow.md`, `docs/release-checklist.md`, and `docs/phase4-reconciliation.md`.

GUI commands, visual acceptance, and installer workflows are outside this pre-GUI CLI document.
