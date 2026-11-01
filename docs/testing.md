# Testing

## Local fast feedback

The local machine has a working Rust toolchain (cargo/rustc with MinGW gcc), Node/npm, and the Shells needed for a useful preflight. Use it for fast feedback before pushing, but treat CI as the cross-platform and release authority.

In Git Bash, export the tool paths and an isolated configuration root before running anything:

```bash
export PATH="/d/mingw64/bin:/d/Program Files/nodejs:$PATH"
export ALIASMGR_CONFIG_DIR="$PWD/target/tcfg"
mkdir -p "$ALIASMGR_CONFIG_DIR"
```

`rusqlite` uses the bundled SQLite build, so `gcc` must be on `PATH`. An injected `ALIASMGR_CONFIG_DIR` is required for `cli_layers`; `config_root.rs` sets its own isolated environment instead of relying on the ambient value.

Local preflight commands:

```bash
cargo check --workspace --all-targets
cargo test --workspace --no-fail-fast
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
cargo test -p aliasmgr-core --lib
cargo test -p aliasmgr-cli --test crud
cargo test -p aliasmgr-tests --test shell_syntax
```

GUI frontend:

```bash
cd crates/aliasmgr-gui/ui
npm ci
npm run typecheck
npm test
npm run build
```

The Tauri Rust crate is excluded from the workspace. It needs `crates/aliasmgr-gui/ui/dist` to exist before `tauri::generate_context!()` can compile, so run the frontend build first:

```bash
cargo test --manifest-path crates/aliasmgr-gui/src-tauri/Cargo.toml
```

Local baseline observed on 2026-11-01 (this branch, before the CI workstream's fmt/clippy pass):

- Workspace: 172 tests passed, 0 failed.
- GUI Tauri crate: 14 tests passed.
- GUI frontend: 215 tests passed; `npm run typecheck` and `npm run build` passed.

Formatting and clippy are owned by the CI workstream and were not re-run as a gate here.

## CI boundary

GitHub Actions remains the only evidence for the Linux/Windows matrix, real Bash/Zsh/PowerShell 5.1/7 behavior, lock contention, packaging, and release artifacts. A local pass is a preflight, not a substitute for a CI run, and no CI success may be claimed for a branch that has not been pushed and run. CI tests must use a temporary HOME, configuration directory, and Profile, and must never modify the runner's real user configuration.

## Historical CI evidence baseline

These runs predate the current branch and are recorded as history, not as evidence for it:

- Fast CI `31875233532`: Linux tests, Windows tests, and lint passed.
- Integration `31875307387`: Bash/Zsh, PowerShell 5.1/7, lock, CLI isolation, and matrix jobs passed.
- Later focused runs are recorded in `plan.md` beside the corresponding Task 8–17 evidence items.

The integration matrix intentionally uses separate Linux Bash/Zsh and Windows PowerShell 5.1/7 jobs. It must not print complete profiles, environment dumps, tokens, or sensitive values.

## What local tests and CI do not prove

Neither local preflight nor CI replaces GUI visual/interaction acceptance, true terminal behavior, real user Profile/ExecutionPolicy compatibility, OneDrive redirection, oh-my-zsh ordering, Windows symlink behavior, package hooks, MSI behavior, or clean-machine installation. Those remain manual acceptance items in `docs/release-checklist.md`.

GUI 的视觉和真实终端行为仍需下载构建产物后人工确认。
