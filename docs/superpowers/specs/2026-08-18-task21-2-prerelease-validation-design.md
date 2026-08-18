# Task 21-2 Prerelease Validation and Beginner Test Design

**Status:** Approved design

## Goal

Provide a beginner-friendly release-validation flow in which CLI and supported Shell checks run through one terminal script per terminal, while GUI visual/runtime checks remain explicit click-through tests. Produce a GitHub prerelease artifact only through an explicit confirmation gate, then collect sanitized result logs and GUI evidence without inferring manual PASS from CI.

## Scope

The validation surface is separated into:

```text
docs/task21-2/
├── README.md
├── test-matrix.md
├── feedback-template.md
├── 01-cli/
├── 02-shell/
├── 03-gui/
└── 04-package/
```

The existing top-level manuals remain compatibility entry points during migration. New content is Chinese, except for required result enum values and command names.

Formal supported Shells:

- Linux Bash
- Linux Zsh
- Windows PowerShell 5.1
- Windows PowerShell 7

Windows Git Bash is optional exploratory coverage only. It must be reported as `NOT-APPLICABLE` or `EXPECTED-LIMITATION` for the formal support matrix because MSYS2 path conversion and Windows native argv behavior are not equivalent to Linux Bash.

## Execution order

The user follows one ordered path:

1. **Prepare:** download prerelease artifacts, verify SHA256, create a disposable result/config directory, and record OS/architecture/Shell/artifact metadata.
2. **CLI artifact smoke:** run version, isolated config, CRUD, search, sync, reload guidance, import/export, and retain/purge checks.
3. **Shell basics:** run the supported terminal script independently for Bash, Zsh, PowerShell 5.1, and PowerShell 7.
4. **Boundary/security:** run argv, placeholder, cwd/environment, import warnings, manual-edit, recovery, target-protection, and upgrade/rollback checks.
5. **Real user configuration:** test login/non-interactive chains, symlink RC/Profile, oh-my-zsh ordering, Profile/OneDrive, ExecutionPolicy, ACL, and native host limitations where applicable.
6. **GUI:** test Windows GUI, then Linux GUI, with explicit click steps for launch, navigation, list/search, wizard, Doctor, import, settings, uninstall, overrides, keyboard, focus, resizing, and visual layout.
7. **Package lifecycle:** install, first launch, upgrade, rollback, retain/purge uninstall, target protection, unmanaged-content preservation, and package hooks.

The order prioritizes low-risk, deterministic checks before environment-sensitive and destructive checks.

## Terminal verification scripts

Prerelease artifacts include one script per formal terminal:

```text
scripts/verify-cli-linux.sh
scripts/verify-bash-linux.sh
scripts/verify-zsh-linux.sh
scripts/verify-powershell51.ps1
scripts/verify-powershell7.ps1
scripts/verify-cli-windows.ps1
scripts/verify-git-bash-windows.sh   # optional exploratory only
```

Examples:

```bash
bash scripts/verify-bash-linux.sh
zsh scripts/verify-zsh-linux.sh
powershell -ExecutionPolicy Bypass -File .\scripts\verify-powershell51.ps1
pwsh -File .\scripts\verify-powershell7.ps1
```

`ExecutionPolicy Bypass`, if used to start the PS5.1 script, applies only to the verification script process. The script must report the real policy and must never modify policy or use bypass to claim Profile success.

Scripts default to temporary directories and must not modify real `.bashrc`, `.zshrc`, PowerShell Profiles, ExecutionPolicy, existing user definitions, or real target files. Any real-profile mode requires a separate explicit switch, a second confirmation, a backup, marked-block-only changes, and a visible backup path.

Each script performs safe automated checks for:

- CLI presence and version;
- Shell/version metadata;
- temporary config and targets;
- add/get/list/update/rename/enable/disable/delete;
- search/fuzzy/field/sort/limit/tag AND;
- empty, quoted, spaced, CJK, backslash, wildcard, and middle-placeholder argv;
- invalid/repeated/disabled placeholders;
- working directory and non-sensitive environment marker;
- sync/reload guidance;
- loader install/uninstall idempotence;
- JSON/TOML preview and confirmed persistence;
- retain/purge target protection;
- cleanup of temporary state.

PowerShell scripts additionally report version, ExecutionPolicy scopes, LanguageMode, Profile path summary, BOM/CRLF summary, and native argv limitations.

## Script outputs and result semantics

Every script produces:

```text
result/verification-summary.json
result/verification.log
result/argv-summary.json
```

The output is sanitized and contains per-case IDs, expected/actual summaries, result enum, evidence path, and `sensitive_data_redacted: true/false`.

Script exit codes:

```text
0  required checks pass; expected limitations may exist
1  implementation/release FAIL
2  environment BLOCKED
3  script argument or preparation error
```

`EXPECTED-LIMITATION` is not silently converted to `PASS`; the summary must distinguish `PASS_WITH_EXPECTED_LIMITATIONS` from all-pass.

## GUI verification

GUI cannot be reduced to terminal output. The user must run the actual Linux/Windows GUI artifact and complete click-through cases. Evidence includes sanitized screenshots, artifact SHA256, config path summary, and any failure log. GUI CI frontend tests do not create GUI manual PASS.

## Prerelease workflow

Add `.github/workflows/prerelease.yml` with only `workflow_dispatch`. Inputs:

- `source_ref`;
- `version`;
- `create_prerelease`;
- exact confirmation string `CREATE-PRERELEASE`.

The workflow:

1. Builds Windows CLI/GUI and Linux CLI/GUI artifacts.
2. Uses fixed runners and isolated config directories.
3. Runs Fast CI, Integration, and Task 23 security gates.
4. Generates SHA256SUMS and a sanitized manifest.
5. Bundles Chinese Task 21-2 manuals and terminal scripts.
6. Marks absent MSI/AppImage/deb/EXE inputs as `environment-blocked`, never as successful installers.
7. Uploads artifacts.
8. Creates a GitHub prerelease only when all automated gates pass, `create_prerelease=true`, and the exact confirmation string matches.
9. Never signs, promotes, merges, publishes a formal release, or performs any action outside the explicit prerelease scope.

Prerelease body states:

- unsigned test build;
- not a production release;
- manual CLI/Shell scripts and GUI click-through required;
- PowerShell 5.1 and 7 are independent;
- Linux Bash/Zsh are formal supported Shells;
- Windows Git Bash is exploratory only;
- feedback must be returned in Chinese;
- no automatic promotion.

## Task 23 gates

Automated gates cover:

- no unsafe `eval`/`Invoke-Expression` in ordinary generated paths;
- explicit `reload --print` command-output exception documentation;
- advanced mode/RawShellCommand rejection;
- generated write-path audit with known manual gaps;
- sensitive import/log/error redaction;
- credential-free URLs and no repository signing material;
- Fast CI, Integration, artifact checksums, and sanitized manifest.

Manual gates remain open for real GUI windows, profiles, OneDrive, ExecutionPolicy, ACL, plugin ordering, installers, package hooks, signing, and release approval.

## Safety and release boundary

No human result is inferred from CI. No prerelease is triggered during implementation without explicit user request. No tag, formal release, signing, merge, or publish is automatic.
