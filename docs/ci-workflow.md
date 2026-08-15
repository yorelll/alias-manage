# CI workflow

CI is the cross-platform verification path because the development environment intentionally has no Rust toolchain. `ci.yml` runs formatting, clippy and workspace tests on Ubuntu 24.04 and Windows. Windows prints PowerShell 5.1 and 7 versions separately. Rust dependencies are cached with `Swatinem/rust-cache`; npm caching belongs in GUI jobs when those jobs are enabled. Tests use temporary HOME/configuration paths and must not modify runner user profiles. `integration.yml` runs the slow process-lock and core integration matrix separately from fast CI.

## Current successful baseline

- Fast CI: `31875233532`
- Integration: `31875307387`

These runs establish the current pre-GUI CLI verification baseline. Focused Task 8–17 runs are linked in `plan.md` beside the relevant subordinate checkboxes.

Use `gh` first. If it is unavailable on PATH, use the absolute GitHub CLI path for this repository:

```text
gh run list --repo yorelll/alias-manage --branch feature/alias-manager-mvp --limit 5
"D:/Program Files/GitHub CLI/gh.exe" run list --repo yorelll/alias-manage --branch feature/alias-manager-mvp --limit 5
"D:/Program Files/GitHub CLI/gh.exe" run watch <run-id> --exit-status
"D:/Program Files/GitHub CLI/gh.exe" run view <run-id> --log-failed
```

Authentication must be performed by the user. CI results do not replace manual GUI, real profile, or terminal behavior checks. Do not claim GUI, installer, real-user-profile, or clean-machine acceptance from a green CLI/core workflow.

## Isolation and logging boundary

Linux jobs use temporary `HOME`, `XDG_CONFIG_HOME`, and `ALIASMGR_CONFIG_DIR`. Windows jobs use temporary `LOCALAPPDATA`, `APPDATA`, and `ALIASMGR_CONFIG_DIR`. The matrix keeps PowerShell 5.1 (`shell: powershell`) separate from PowerShell 7 (`shell: pwsh`). Logs must not dump environment variables, complete profiles, tokens, or sensitive values.

See `docs/release-checklist.md` for the separation between CI evidence and manual acceptance.

## Deferred jobs

GUI npm cache, Tauri build jobs, package builds, signing, and installer tests are intentionally deferred until the GUI/release phase. `release.yml` is a future release boundary and must not be treated as successful merely because the current fast/integration workflows pass.
