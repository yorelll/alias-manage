# M0 + M1 Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Establish a CI-verifiable Rust workspace with a coherent alias domain model, validation, SQLite CRUD storage, and CLI parsing foundation.

**Architecture:** `aliasmgr-core` owns model types, validation, migrations, and storage; `aliasmgr-cli` depends on the core crate and only owns command parsing. The workspace root contains only shared configuration and the CI workflow. The GUI scaffold stays outside the workspace until its Tauri dependencies are intentionally introduced.

**Tech Stack:** Rust stable, Cargo workspace, Serde, SQLite via `rusqlite` with `bundled`, `uuid`, `chrono`, `thiserror`, `clap`, GitHub Actions.

---

## Files and responsibilities

- Modify `Cargo.toml`: define only the three compilable workspace members and shared dependency versions.
- Modify `rust-toolchain.toml`: pin stable toolchain components needed by CI.
- Modify `crates/aliasmgr-core/Cargo.toml`: declare core dependencies.
- Modify `crates/aliasmgr-core/src/lib.rs`: export core modules and keep the version smoke test.
- Modify `crates/aliasmgr-core/src/model.rs`: define serializable alias domain types and defaults.
- Modify `crates/aliasmgr-core/src/error.rs`: define stable core error variants.
- Modify `crates/aliasmgr-core/src/validation.rs`: validate names, shells, executables, advanced mode, and argument placeholders.
- Modify `crates/aliasmgr-core/src/migrations/0001_initial.sql`: create the minimal schema required by CRUD tests.
- Modify `crates/aliasmgr-core/src/migrations/mod.rs`: expose migration SQL and schema version.
- Modify `crates/aliasmgr-core/src/storage.rs`: implement database open, migration, insert, get, list, update, delete, and transaction rollback behavior.
- Modify `crates/aliasmgr-cli/Cargo.toml`: add `clap` and core dependencies for parser tests.
- Modify `crates/aliasmgr-cli/src/main.rs`: use the shared parser module and return stable usage errors.
- Create `crates/aliasmgr-cli/src/cli.rs`: define global options and the foundational command tree.
- Create `crates/aliasmgr-cli/tests/parse.rs`: test command parsing without invoking storage or shells.
- Modify `.github/workflows/ci.yml`: run formatting, clippy, Linux tests, and Windows tests against the valid workspace.
- Modify `docs/ci-workflow.md`: document the first remote verification loop and the exact `gh.exe` commands.
- Modify `README.md`: document the current foundation milestone and CI-only Rust verification.

---

### Task 1: Make the workspace compile as a minimal workspace

**Files:** `Cargo.toml`, `rust-toolchain.toml`, `crates/aliasmgr-core/Cargo.toml`, `crates/aliasmgr-cli/Cargo.toml`, `crates/aliasmgr-tests/Cargo.toml`

- [x] **Step 1: Remove non-workspace GUI code from the Cargo workspace.** Keep members exactly `crates/aliasmgr-core`, `crates/aliasmgr-cli`, and `crates/aliasmgr-tests`; the GUI Cargo manifest remains a future scaffold and must not be compiled yet.
- [x] **Step 2: Ensure all workspace dependency declarations use valid names and versions.** The root must include `chrono`, `clap`, `rusqlite` with `features = ["bundled"]`, `serde`, `serde_json`, `thiserror`, and `uuid`.
- [x] **Step 3: Add a workspace lint policy.** Configure Rust warnings as errors in CI through clippy rather than crate-specific duplicated settings.
- [x] **Step 4: Update each member manifest to use `workspace = true` for version, edition, and license.** Keep `aliasmgr-cli` dependent on `aliasmgr-core`; keep `aliasmgr-tests` as a non-published dev-only integration crate.
- [x] **Step 5: Review manifests statically.** Confirm every dependency referenced by source is declared by that crate or inherited from the workspace.
- [x] **Step 6: Commit the workspace correction.** Use `git add Cargo.toml rust-toolchain.toml crates/*/Cargo.toml && git commit -m "chore: make rust workspace buildable"`.

### Task 2: Complete the domain model and error contract

**Files:** `crates/aliasmgr-core/src/model.rs`, `crates/aliasmgr-core/src/error.rs`, `crates/aliasmgr-core/src/lib.rs`

- [x] **Step 1: Add model round-trip tests before changing implementations.** Test `AliasRecord::default()` serializes and deserializes with UUID and RFC3339 timestamps, and test unknown enum strings are rejected by Serde.
- [x] **Step 2: Define all MVP model enums with `serde(rename_all = "snake_case")`:** `TargetType`, `ShellKind`, `PathMode`, and `PathOrigin`.
- [x] **Step 3: Define `AliasRecord` with the plan fields:** UUID, name, description, target type, executable, fixed argument array, pass-through flag, optional working directory, ordered environment map, shells, enabled flag, advanced mode flag, tags, path metadata, timestamps, checksum, and revision.
- [x] **Step 4: Define `Conflict`, `ShellState`, `ManagedNameSet`, and `SyncReceipt` as serializable types sufficient for later modules.** `SyncReceipt` must carry per-shell status rather than only one aggregate boolean.
- [x] **Step 5: Define core errors for invalid names, invalid argument templates, advanced mode, alias conflicts, reserved names, missing targets, database, serialization, and I/O failures.** Preserve error text in English; user-facing translation remains a later CLI layer.
- [x] **Step 6: Export the modules from `lib.rs` and keep tests in the module that owns each type.**
- [x] **Step 7: Commit the model contract.** Use `git add crates/aliasmgr-core/src && git commit -m "feat: add alias domain model"`.

### Task 3: Implement validation with focused unit tests

**Files:** `crates/aliasmgr-core/src/validation.rs`, `crates/aliasmgr-core/src/model.rs`

- [x] **Step 1: Write table-driven tests for valid names `cm`, `copy-mv`, `build_cam`, and `g1`; invalid names `my alias`, `a=b`, `hello;world`, `$cmd`, empty names, and names longer than 64 characters.**
- [x] **Step 2: Write placeholder tests for no placeholder with pass-through, one `{{args}}`, a placeholder with `pass_args = false`, duplicate placeholders, and escaped literal `{{{{args}}}}`.**
- [x] **Step 3: Implement `validate_alias_name(&str) -> Result<(), AliasError>` using the cross-platform ASCII rule `^[A-Za-z_][A-Za-z0-9_-]*$` and a 64-character maximum.**
- [x] **Step 4: Implement `validate_alias(&AliasRecord) -> Result<(), AliasError>` checking nonempty executable, nonempty shells, advanced mode rejection, RawShellCommand rejection, and placeholder semantics.**
- [x] **Step 5: Add the PowerShell reserved-name check as a separate function, with a documented small initial set (`ls`, `cp`, `mv`, `rm`, `cat`, `gc`) that can be expanded later when real PowerShell discovery is implemented.**
- [x] **Step 6: Commit the validation contract.** Use `git add crates/aliasmgr-core/src/model.rs crates/aliasmgr-core/src/validation.rs crates/aliasmgr-core/src/error.rs && git commit -m "feat: validate alias definitions"`.

### Task 4: Implement minimal SQLite migration and CRUD

**Files:** `crates/aliasmgr-core/src/migrations/0001_initial.sql`, `crates/aliasmgr-core/src/migrations/mod.rs`, `crates/aliasmgr-core/src/storage.rs`, `crates/aliasmgr-core/src/lib.rs`

- [x] **Step 1: Write in-memory database tests for migration, insert, get by ID, get by name, list ordering, update, delete, and rollback.** Use `Connection::open_in_memory()` or a test constructor; do not write into a real user directory.
- [x] **Step 2: Expand migration SQL to create `schema_migrations` and the `aliases` table with the plan’s JSON columns:** fixed args, environment, shells, tags, path fields, timestamps, checksum, and revision. Add unique binary name and folded-name indexes.
- [x] **Step 3: Implement `Database::open(path)` with `foreign_keys = ON`, `busy_timeout = 5000`, and a migration call.** Add a test-only `from_connection` constructor so unit tests stay in memory.
- [x] **Step 4: Implement a migration executor that records version and SQL checksum and rejects a database whose `PRAGMA user_version` exceeds the known version.** Return a dedicated `SchemaTooNew` error.
- [x] **Step 5: Implement Serde conversion helpers for the JSON columns and explicit integer conversion for booleans.** Store timestamps as RFC3339 strings and UUIDs as strings.
- [x] **Step 6: Implement `insert_alias`, `get_alias`, `get_alias_by_name`, `list_aliases`, `update_alias`, and `delete_alias`.** Map unique-name violations to `AliasConflict`; preserve the caller’s original name while storing `name_folded`.
- [x] **Step 7: Add a transaction helper test that inserts a row, returns an error, rolls back, and verifies the row is absent.**
- [x] **Step 8: Commit storage.** Use `git add crates/aliasmgr-core/src/storage.rs crates/aliasmgr-core/src/migrations crates/aliasmgr-core/src/error.rs && git commit -m "feat: add transactional sqlite storage"`.

### Task 5: Add a minimal CLI parser contract

**Files:** `crates/aliasmgr-cli/src/cli.rs`, `crates/aliasmgr-cli/src/main.rs`, `crates/aliasmgr-cli/tests/parse.rs`, `crates/aliasmgr-cli/Cargo.toml`

- [x] **Step 1: Write parser tests for `add cm --exec python3 --arg copymv.py --shell bash`, `remove cm --yes`, `find query --fuzzy --limit 10`, `list --sort updated_at --desc`, `sync --dry-run`, `reload --print`, `doctor`, `shell detect`, `import file.json`, `export file.json`, and `uninstall --purge-aliases`.**
- [x] **Step 2: Define `Cli` with global `--config-dir`, `--format table|json`, `--no-color`, `--verbose`, and `--quiet`.**
- [x] **Step 3: Define the foundational `Command` enum and typed arguments, including repeatable `--arg`, `--env`, and `--tag`, `--cwd`, `--shell`, `--limit`, `--dry-run`, and `--yes`.**
- [x] **Step 4: Make `main.rs` parse the command and print only a temporary foundation message; do not access SQLite directly from `main.rs`.**
- [x] **Step 5: Commit the parser.** Use `git add crates/aliasmgr-cli && git commit -m "feat: add alias manager cli command model"`.

### Task 6: Establish and run the first remote CI loop

**Files:** `.github/workflows/ci.yml`, `docs/ci-workflow.md`, `README.md`

- [x] **Step 1: Update CI to use fixed action versions, `ubuntu-24.04`, `windows-latest`, concurrency cancellation, timeout limits, and separate PowerShell 5.1/7 version-print steps.**
- [ ] **Step 2: Ensure Linux runs `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace`; Windows runs `cargo test --workspace`.**
- [ ] **Step 3: Add CI documentation describing that local Cargo is prohibited by the project plan, that tests use isolated temporary directories, and that remote checks use the absolute GitHub CLI path.**
- [ ] **Step 4: Confirm the remote repository target with `git remote -v`; do not rewrite remotes or change repository settings without user authorization.**
- [ ] **Step 5: Push the feature branch with `git push -u origin feature/alias-manager-mvp`.** This is explicitly authorized by the user for this iteration.
- [ ] **Step 6: Find the run with `"D:/Program Files/GitHub CLI/gh.exe" run list --branch feature/alias-manager-mvp --limit 5`.**
- [ ] **Step 7: Wait for the newest run with `"D:/Program Files/GitHub CLI/gh.exe" run watch <run-id> --exit-status`.**
- [ ] **Step 8: If CI fails, inspect only failure logs with `"D:/Program Files/GitHub CLI/gh.exe" run view <run-id> --log-failed`, fix the concrete cause, create a new semantic commit, push, and repeat. Stop after three failures with the same root cause and report the blocker.**
- [ ] **Step 9: Record the successful run ID and job result in the final report.**

## Verification boundary

Do not run `cargo`, `rustc`, `rustup`, Bash/Zsh/PowerShell, or Tauri commands locally. The first evidence for Rust compilation and tests must come from GitHub Actions. Local checks are limited to file inspection, `git`, the absolute `gh.exe`, `npm` if available, and `python`.
