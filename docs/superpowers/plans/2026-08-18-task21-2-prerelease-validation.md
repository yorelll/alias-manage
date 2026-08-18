# Task 21-2 Prerelease Validation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver beginner-friendly Chinese CLI/Shell/GUI/package acceptance materials, one safe verification script per supported terminal, Task 23 security gates, and a manually confirmed GitHub prerelease artifact flow.

**Architecture:** Keep manual acceptance separate from automated evidence. CLI and Shell scripts run deterministic artifact smoke tests in temporary directories and emit sanitized JSON/log results; GUI and visual/runtime checks remain click-through manuals. A workflow builds unsigned artifacts and only creates a GitHub prerelease when an explicit `CREATE-PRERELEASE` confirmation matches after automated gates pass.

**Tech Stack:** Bash, Zsh, PowerShell 5.1/7, Rust CLI/core, Tauri 2, GitHub Actions, SHA256SUMS, sanitized JSON manifests, GitHub CLI.

---

## Global constraints

- Do not run local Cargo, Rust, Bash, Zsh, PowerShell, Tauri, or GUI server commands.
- Verify Rust/Shell/Tauri behavior only through GitHub Actions.
- Use `gh` first and `"D:/Program Files/GitHub CLI/gh.exe"` as fallback.
- Do not trigger the prerelease workflow during implementation without explicit user instruction.
- Do not create a formal tag/release, sign, merge, or publish automatically.
- Never infer a manual `PASS` from CI.
- Keep Windows Git Bash exploratory only; it cannot substitute for Linux Bash.
- Do not modify the user’s real Profile/RC/ExecutionPolicy by default.

## File map

**Create:**

- `scripts/verify-cli-linux.sh` — Linux CLI artifact smoke runner.
- `scripts/verify-bash-linux.sh` — Bash-specific isolated runner.
- `scripts/verify-zsh-linux.sh` — Zsh-specific isolated runner.
- `scripts/verify-powershell51.ps1` — Windows PowerShell 5.1 runner.
- `scripts/verify-powershell7.ps1` — Windows PowerShell 7 runner.
- `scripts/verify-cli-windows.ps1` — Windows CLI artifact runner shared by PS versions.
- `scripts/verify-git-bash-windows.sh` — optional exploratory Git Bash runner.
- `.github/workflows/prerelease.yml` — manually confirmed unsigned prerelease workflow.
- `docs/task21-2/01-cli/README.md` — Chinese beginner CLI entry point.
- `docs/task21-2/01-cli/linux-cli.md` — Linux CLI ordered steps.
- `docs/task21-2/01-cli/windows-cli.md` — Windows CLI ordered steps.
- `docs/task21-2/01-cli/artifact-smoke.md` — artifact download/checksum/run instructions.
- `docs/task21-2/02-shell/README.md` — Shell execution order and support scope.
- `docs/task21-2/02-shell/linux-bash.md` — Bash instructions.
- `docs/task21-2/02-shell/linux-zsh.md` — Zsh instructions.
- `docs/task21-2/02-shell/windows-powershell51.md` — PS5.1 instructions.
- `docs/task21-2/02-shell/windows-powershell7.md` — PS7 instructions.
- `docs/task21-2/02-shell/windows-git-bash.md` — exploratory limitation instructions.
- `docs/task21-2/03-gui/README.md` — GUI beginner order and evidence rules.
- `docs/task21-2/03-gui/windows-gui.md` — Windows GUI click-through.
- `docs/task21-2/03-gui/linux-gui.md` — Linux GUI click-through.
- `docs/task21-2/04-package/README.md` — package testing order.
- `docs/task21-2/04-package/windows-installer.md` — MSI/EXE steps and missing-artifact handling.
- `docs/task21-2/04-package/linux-package.md` — AppImage/deb steps and missing-artifact handling.
- `docs/final-release-status.md` — update with prerelease artifact and human report status.

**Modify:**

- `docs/task21-2/README.md` — link the ordered CLI/Shell/GUI/package directories.
- `docs/task21-2/test-matrix.md` — preserve all IDs, add script result-log columns/links, keep blank until human execution.
- `docs/task21-2/feedback-template.md` — preserve result enum and add script/log metadata fields.
- `docs/superpowers/plans/2026-08-17-task21-2-prerelease-validation-design.md` — link implementation outcomes.
- `docs/superpowers/plans/2026-08-17-task21-2-prerelease-validation.md` — mark steps with evidence.
- `plan.md` — update Task 23 and release boundary only with evidence.
- `docs/release-checklist.md` — add prerelease artifact and Task 23 gate status.
- `docs/ci-workflow.md` — document prerelease inputs, artifacts, confirmation, and no-publish boundary.

---

### Task 1: Reorganize Chinese beginner manuals

**Files:** the new `docs/task21-2/01-cli/`, `02-shell/`, `03-gui/`, `04-package/` files plus existing README/matrix/template.

- [ ] **Step 1: Write the ordered top-level guide**

Coverage check: the guide must link all four directories, distinguish CLI artifact smoke from Shell scripts, put GUI after CLI/Shell, and put package lifecycle last.

Update `docs/task21-2/README.md` with these exact beginner steps: prepare artifact/checksum, run CLI artifact smoke, run supported Shell scripts, run edge/security checks, test real user configuration, test Windows GUI, test Linux GUI, then test package lifecycle. State that CLI CI covers deterministic tests but artifact smoke is still required on the user machine.

- [ ] **Step 2: Write separate CLI manuals**

Each CLI manual must show where to open a terminal, where to unpack the artifact, how to run the one command, what files are produced, how to read `verification-summary.json`, and how to submit a result log. Include explicit cleanup steps and never ask a beginner to paste a full environment dump.

- [ ] **Step 3: Write separate Shell manuals**

Document Bash, Zsh, PS5.1, and PS7 independently. Include exact terminal selection, script command, expected summary, manual-only follow-up cases, and Git Bash exploratory classification.

- [ ] **Step 4: Write GUI/package manuals**

Use numbered click paths such as “点击左侧 Aliases → 点击 Add → 在 Name 输入框输入 … → 点击 Preview → 点击 Save”. Every GUI case must specify screenshot points, expected UI state, and what to do on error. Package manuals must explain how to classify missing MSI/AppImage/deb/EXE as `NOT-APPLICABLE` or `BLOCKED`.

- [ ] **Step 5: Update matrix/template without results**

Preserve the IDs and exact enum values. Add fields for script summary path, log path, artifact SHA256, and screenshot path. Keep all human results blank.

- [ ] **Step 6: Commit manuals**

```bash
git add docs/task21-2
git commit -m "docs: reorganize beginner task21-2 manuals"
git push origin feature/alias-manager-mvp
```

---

### Task 2: Implement shared sanitized result format

**Files:** all six terminal scripts, `docs/task21-2/feedback-template.md`, `docs/ci-test-inventory.md`.

- [ ] **Step 1: Write script contract tests**

Add repository-safe source-contract tests that assert every script declares output paths `result/verification-summary.json`, `result/verification.log`, and `result/argv-summary.json`; uses the exact enum values; creates a temporary config root; and contains no environment-dump command.

- [ ] **Step 2: Implement common result schema in each script**

Each result item must contain `id`, `title`, `result`, `expected`, `actual`, `evidence`, and `sensitive_data_redacted`. The final summary must contain counts and `PASS_WITH_EXPECTED_LIMITATIONS` when appropriate. Use exit codes 0/1/2/3 exactly as specified in the design.

- [ ] **Step 3: Implement temporary-root cleanup**

Default scripts create temporary `config`, `profile`, `generated`, `targets`, and `result` directories. Register cleanup on normal exit and preserve only the result directory unless the user explicitly requests retention.

- [ ] **Step 4: Commit script contract**

```bash
git add scripts docs/task21-2/feedback-template.md
git commit -m "feat: add sanitized terminal verification scripts"
git push origin feature/alias-manager-mvp
```

---

### Task 3: Implement Linux CLI/Bash/Zsh scripts

**Files:** `scripts/verify-cli-linux.sh`, `scripts/verify-bash-linux.sh`, `scripts/verify-zsh-linux.sh`.

- [x] **Step 1: Write source-contract tests**

Assert the Linux scripts check version, isolated config, CRUD, search/tag, argv, placeholders, import/export, target protection, loader idempotence, and cleanup; assert no `set -x`, `printenv`, or complete profile output. Evidence: `crates/aliasmgr-gui/ui/src/script-contracts.test.ts` and Fast CI run `32118998771`.

- [x] **Step 2: Implement Linux CLI runner**

Accept `--artifact PATH` and optional `--keep-temp`. Resolve the CLI binary without modifying PATH globally. Create a disposable target/dumper and run ordered cases L-001 through L-019 that are safe to automate. Write only summaries and sanitized paths. Evidence: `scripts/verify-cli-linux.sh` with CRUD update, wildcard argv, environment isolation, target protection, import/export, loader idempotence, and sanitized result outputs.

- [x] **Step 3: Implement Bash and Zsh runners**

Accept `--artifact PATH` and use the script’s own temporary RC paths. Run loader install/reload/idempotence, syntax, argv, tag/search, tombstone, and uninstall checks. Leave real login-chain, oh-my-zsh order, trusted symlink, and current-session manual checks clearly marked for the user. Evidence: `scripts/verify-bash-linux.sh` and `scripts/verify-zsh-linux.sh`; manual-only cases remain explicitly classified.

- [x] **Step 4: Verify scripts through CI**

Add a Linux workflow job that runs the scripts against CI-built CLI artifacts or a workspace binary, then run Fast CI and Integration through `gh`. No local Bash/Zsh/Cargo execution is allowed. Evidence: Integration run `32119029002` passed all 12 matrix jobs; Fast CI run `32118998771` passed the script contract checks.

- [x] **Step 5: Commit Linux scripts**

Evidence commits: `01e6138`, `aac66da`, `fc90704`, `e8b69af`, `4e2e897`, `8a2b520`; integration merge `f834c23`.

```bash
git add scripts .github/workflows/integration.yml
git commit -m "feat: add linux terminal verification scripts"
git push origin feature/alias-manager-mvp
```

---

### Task 4: Implement Windows CLI/PowerShell scripts

**Files:** `scripts/verify-cli-windows.ps1`, `scripts/verify-powershell51.ps1`, `scripts/verify-powershell7.ps1`.

- [x] **Step 1: Write source-contract tests**

Assert each script reports its exact Shell version, policy scopes, LanguageMode, sanitized Profile summary, BOM/CRLF summary, and native argv limitation; assert PS5.1 and PS7 scripts are independent. Added Group Policy reporting, Profile-via-bypass enforcement, cross-shell-invocation prevention, forbidden policy/evaluation scans, and output-contract checks in `crates/aliasmgr-gui/ui/src/script-contracts.test.ts`.

- [x] **Step 2: Implement shared Windows CLI runner**

Accept `-ArtifactPath`, `-OutputRoot`, and `-KeepTemp`. Use temporary `LOCALAPPDATA`, `APPDATA`, config, profile, target, and result roots. Run CRUD/search/tag/argv/placeholder/import/uninstall/target-protection cases without modifying the real profile. Evidence: `scripts/verify-cli-windows.ps1`.

- [x] **Step 3: Implement PS5.1 and PS7 wrappers**

Each wrapper records its own version and invokes only its own Shell. Report Restricted/AllSigned/Group Policy/ConstrainedLanguage as explicit statuses; never change policy and never claim Profile success via bypass. Evidence: `scripts/verify-powershell51.ps1` and `scripts/verify-powershell7.ps1`.

- [x] **Step 4: Verify scripts in separate CI jobs**

Run PS5.1 with `shell: powershell` and PS7 with `shell: pwsh`; verify result JSON is sanitized. Use GitHub Actions only. Integration run `32137450742` passed all 14 jobs, including `windows_verify_scripts_ps51` and `windows_verify_scripts_ps7`; Fast CI run `32133072318` passed all five jobs.

- [x] **Step 5: Commit Windows scripts**

Implemented and reviewed on `feature/alias-manager-mvp`; no release/tag/sign/publish action was performed.

**Known CLI limitation (L-003d / `update` command):**
The CLI `update NAME` command accepts only the alias name at the CLI interface level; `--exec`/`--arg` are not exposed as CLI flags. The Linux verifier tests the supported `update gs` interface without claiming unexposed argv mutation.

**CI evidence:**
- Fast CI run `32133072318`: all five jobs passed.
- Integration run `32137450742`: all 14 jobs passed, including Linux CLI/Bash/Zsh and independent Windows PS5.1/PS7 verification jobs.

---

### Task 5: Add explicit prerelease workflow

**Files:** `.github/workflows/prerelease.yml`, `docs/ci-workflow.md`, `docs/release-checklist.md`, `docs/final-release-status.md`.

- [ ] **Step 1: Write workflow contract checks**

Add source-contract checks asserting the workflow is `workflow_dispatch` only, defines `source_ref`, `version`, `create_prerelease`, and confirmation inputs, requires exact `CREATE-PRERELEASE`, uploads unsigned artifacts, creates no Release unless explicitly requested, and contains no signing/publish/force-push step.

- [ ] **Step 2: Add Linux and Windows build jobs**

Build CLI binaries on fixed Linux/Windows runners. Attempt Tauri GUI build using fixed native dependencies. If MSI/AppImage/deb/EXE inputs are absent or native compilation fails, write `environment-blocked` to the manifest and do not fabricate an installer.

- [ ] **Step 3: Add gate and artifact jobs**

Run Fast CI, Integration, Task 23 source scan, terminal script contract tests, and GUI frontend checks. Build `SHA256SUMS`, sanitized `manifest.json`, and bundle Chinese manuals/scripts.

- [ ] **Step 4: Add explicit prerelease creation job**

Condition creation on all automated jobs passing, `create_prerelease == true`, and `confirmation == 'CREATE-PRERELEASE'`. Use least-privilege `contents: write` only on the creation job. Use `gh release create --prerelease` or the GitHub release action only inside this condition. Never sign or publish a formal release.

- [ ] **Step 5: Verify safely without creating a Release**

Run the workflow with `create_prerelease=false` and a non-matching confirmation. Confirm artifacts/status jobs pass and creation is skipped. Do not trigger the creation path in this implementation task.

- [ ] **Step 6: Commit prerelease workflow**

```bash
git add .github/workflows/prerelease.yml docs/ci-workflow.md docs/release-checklist.md docs/final-release-status.md
git commit -m "ci: prepare manually confirmed prerelease artifacts"
git push origin feature/alias-manager-mvp
```

---

### Task 6: Complete Task 23 release gates

**Files:** `plan.md`, `docs/release-checklist.md`, `docs/security.md`, `docs/limitations.md`, `.github/workflows/prerelease.yml`.

- [ ] **Step 1: Add failing/source contract checks for Task 23**

Check ordinary generated paths for unsafe evaluation, advanced-mode rejection, write-boundary markers, sensitive redaction, credential-free URLs, and no signing material.

- [ ] **Step 2: Implement checks and evidence collection**

Emit sanitized gate summaries containing gate ID, result, evidence path, and manual gap. Explicitly separate automated PASS from manual pending.

- [ ] **Step 3: Run all automated gates remotely**

Run Fast CI, Integration, Release dry-run, and prerelease dry-run via gh. Record run IDs, job names, test inventory, artifact names, SHA256, failures/fixes, and manual gaps.

- [ ] **Step 4: Commit Task 23 evidence**

```bash
git add plan.md docs/release-checklist.md docs/security.md docs/limitations.md docs/final-release-status.md
git commit -m "release: verify task23 prerelease gates"
git push origin feature/alias-manager-mvp
```

---

### Task 7: Final handoff and manual report request

**Files:** `docs/final-release-status.md`, `docs/task21-2/test-matrix.md`, `docs/task21-2/feedback-template.md`.

- [ ] Update final status with prerelease URL only after the user explicitly authorizes and the creation workflow succeeds.
- [ ] Provide the user the exact download/checksum/script commands for Windows 11 PS5.1, Windows 11 PS7, Linux Bash, Linux Zsh, Windows GUI, and Linux GUI.
- [ ] Keep all manual matrix results blank until the user returns logs/screenshots.
- [ ] Do not create formal release/tag/sign/publish/merge actions.
