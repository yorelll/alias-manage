# CI workflow

CI is the cross-platform verification path because the development environment intentionally has no Rust toolchain. `ci.yml` runs formatting, clippy and workspace tests on Ubuntu 24.04 and Windows. Windows prints PowerShell 5.1 and 7 versions separately. Tests must use temporary configuration directories and must not modify runner user profiles.

Use the absolute GitHub CLI path for this repository:

```text
"D:/Program Files/GitHub CLI/gh.exe" run list --branch feature/alias-manager-mvp --limit 5
"D:/Program Files/GitHub CLI/gh.exe" run watch <run-id> --exit-status
"D:/Program Files/GitHub CLI/gh.exe" run view <run-id> --log-failed
```

Authentication must be performed by the user. CI results do not replace manual GUI, real profile, or terminal behavior checks.
