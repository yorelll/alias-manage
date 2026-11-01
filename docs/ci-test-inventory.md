# Automated CI Test Inventory

This inventory maps stable behavior IDs to the focused test target and workflow job. It records automated evidence only; manual/profile/visual acceptance remains in `docs/task21-2/`.

| ID range | Behavior area | Test target | Workflow job |
|---|---|---|---|
| M-001–M-006 | model, serialization, checksum, enum rejection | `aliasmgr-core` model tests | CI `test-linux`, `test-windows` |
| V-001–V-009 | name, argument template, reserved name, advanced mode validation | `aliasmgr-core` validation tests | CI `test-linux`, `test-windows` |
| S-001–S-014 | SQLite CRUD, migration, schema, conflict, shell state, overrides | `aliasmgr-core` storage tests | CI `test-linux`, `test-windows` |
| Q-001–Q-009 | scoring, fuzzy search, fields, tags, sorting, limits | `aliasmgr-core` search tests; CLI parse/crud | CI `lint`, `test-linux`, `test-windows` |
| E-001–E-011 | structured argv, placeholders, environment/cwd, Batch safety | `aliasmgr-core` executor tests; argument fixtures | CI `test-linux`, `test-windows`, Integration |
| B-001–B-006 | Bash rendering, quoting, preemption, loader, syntax | core shell tests; `shell_syntax`, `bash` | Integration Bash jobs |
| Z-001–Z-004 | Zsh rendering, loader, syntax | core shell tests; `zsh`, `shell_syntax` | Integration Zsh job |
| P5-001–P5-006 | PowerShell 5.1 rendering, parser, preemption | `aliasmgr-tests` PowerShell tests | Integration PS 5.1 job |
| P7-001–P7-006 | PowerShell 7 rendering, parser, preemption | `aliasmgr-tests` PowerShell tests | Integration PS 7 job |
| Y-001–Y-009 | sync, rollback, journal, shell state, forward recovery | `aliasmgr-core` sync tests | CI and Integration |
| C-001–C-012 | CLI lifecycle, output, diagnostics, transfer, uninstall | `aliasmgr-cli` integration tests | CI and Integration CLI jobs |
| G-001–G-012 | GUI frontend contracts, panels, commands, invoke args | GUI Node tests | CI GUI Linux/Windows jobs |
| G-RUST (14 tests) | Tauri commands, DTO/checksum behavior, state scoping | `aliasmgr-gui` crate unit tests | `ci.yml` `gui-rust-windows`: frontend build, crate `cargo fmt -- --check`, `cargo check --locked`, `cargo test --locked`; not yet run for this branch |
| F-001–F-010 | failure paths and redaction | validation/storage/transfer/diagnostics tests | CI and Integration |
| N-001–N-003 | isolation, no environment dumps, no secret fixture output | `cli_layers`, workflow isolation assertions | Integration jobs |
| PERF-001 | 500/1000 alias generation benchmark | `aliasmgr-tests --test benchmark` | Integration/benchmark job |

## Phase B evidence

- Fast CI `32093176327`: Linux/Windows workspace, lint, GUI npm cache with `npm ci`, and frontend tests/typecheck/build.
- Integration `32093828288`: Bash, Zsh, oh-my-zsh contract, PowerShell 5.1/7, lock, CLI, benchmark, core, and isolated configuration jobs.
- Release `32093979406`: dry-run packaging status, unsigned source archive, SHA256SUMS, sanitized manifest, and source security scan.

Failure-path coverage includes invalid names, missing targets, exact/case-fold conflicts, checksum mismatch, permission/unsafe-path results, lock timeout, malformed imports, unsupported records, partial sync, and purge target protection. The inventory does not claim PowerShell Profile/ExecutionPolicy, OneDrive, oh-my-zsh ordering, installers, or real GUI windows.

## Coverage additions (2026-11-01, local evidence only)

These map the newer behaviors to the tests that exercise them. The branch has merged main (`52e0635`) and `ci.yml` now contains the `gui-rust-windows` job (frontend build, Tauri crate rustfmt check, `cargo check --locked`, `cargo test --locked`), but no CI run has been triggered for this branch. Local results are preflight, not CI evidence.

| Behavior | Local test target | Result |
|---|---|---|
| Configuration root precedence (`--config-dir` > non-empty `ALIASMGR_CONFIG_DIR` > platform default) | `crates/aliasmgr-cli/tests/config_root.rs` | 1 passed |
| Revision monotonicity across update/delete/tombstones | `crates/aliasmgr-cli/tests/revisions.rs`; core `sync` tests | 3 passed |
| `update` field preservation, NotFound = 4, invalid shell/env = 1 | `crates/aliasmgr-cli/tests/crud.rs`, `parse.rs` | 14 passed combined |
| `import` preview vs `--yes`, sensitive environment filtering | `crates/aliasmgr-cli/tests/transfer.rs`, `import_env.rs`; core `transfer.rs` | 6 passed combined |
| `uninstall` retain/purge, loader removal, target-file protection | `crates/aliasmgr-cli/tests/uninstall.rs`; core `uninstall.rs`; `scripts/verify-cli-windows.ps1` W-008 | 3 passed + W-008 PASS locally |
| Syntax check boundary and previous-file preservation | `crates/aliasmgr-core/src/syntax.rs` tests; `sync.rs::syntax_failure_preserves_previous_generated_file_and_leaves_no_temp` | passed |
| PowerShell 5.1 BOM default, PS7 BOM opt-in, checksum BOM stripping | `crates/aliasmgr-core/src/sync.rs` BOM tests; `config.rs` BOM defaults | passed |
| GUI Rust commands/DTO/checksum | `aliasmgr-gui` crate unit tests | 14 passed |
| GUI frontend wiring, search/tags, wizard, uninstall panel, script contracts, task20 regressions | `crates/aliasmgr-gui/ui/src/{app,wiring,script-contracts,task20}.test.ts` | 223 passed; typecheck/build passed |
