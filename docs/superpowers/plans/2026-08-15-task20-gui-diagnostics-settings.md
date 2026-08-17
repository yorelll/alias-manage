# Task 20 GUI Diagnostics, Transfer, and Settings Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. This plan is executed inline in the current agent; do not dispatch subagents.

**Goal:** Add core-backed Doctor, import preview/confirmation, limited Settings, overridden-definition display, and Retain/Purge uninstall controls to the Task 18/19 GUI shell.

**Architecture:** Tauri commands remain thin DTO adapters over existing `aliasmgr-core` detection, transfer, config, service, sync, storage, and uninstall APIs. React panels own drafts, confirmation state, and presentation only; core owns validation, persistence, severity/status, safety filtering, and target protection. The standalone GUI crate remains outside the root Cargo workspace.

**Tech Stack:** Rust, `aliasmgr-core`, rusqlite, Serde, Tauri 2 commands, React/TypeScript, Node test, Vite, GitHub Actions.

---

## Scope and verification boundary

- Implement Task 20 in the current agent only; do not dispatch subagents.
- Do not run local Cargo, Rust, Shell, Tauri, or GUI dev-server commands. Verify Rust/frontend through GitHub Actions only.
- Task 20 includes Doctor, generated preview/reload copy, safe refresh action boundary, import preview/confirm, limited Settings, overridden definitions display/copy, and Retain/Purge uninstall confirmation.
- Do not implement automatic Profile/RC edits, automatic ExecutionPolicy changes, automatic override restoration, MSI/package hooks, visual acceptance, real window startup, or clean-machine acceptance.
- Keep the original Task 20 broad checkbox unchecked until implementation, CI, and manual-boundary evidence are complete.

## File map

- Create `crates/aliasmgr-gui/ui/src/components/DoctorPanel.tsx`: severity summary, per-Shell details, safe/manual actions, generated preview and reload copy.
- Create `crates/aliasmgr-gui/ui/src/components/ImportPanel.tsx`: single-page import preview and explicit confirmation.
- Create `crates/aliasmgr-gui/ui/src/components/SettingsPanel.tsx`: draft settings, read-only config path, unified save.
- Create `crates/aliasmgr-gui/ui/src/components/UninstallPanel.tsx`: Retain/Purge selection, risk summary, second confirmation.
- Modify `crates/aliasmgr-gui/ui/src/App.tsx`: route Doctor/Settings pages and panel state.
- Modify `crates/aliasmgr-gui/ui/src/lib.ts`: Task 20 DTOs, command adapter helpers, confirmation/presentation helpers.
- Modify `crates/aliasmgr-gui/src-tauri/src/commands.rs`: thin Task 20 commands and serialized response types.
- Modify `crates/aliasmgr-gui/src-tauri/src/main.rs`: register Task 20 commands.
- Create `crates/aliasmgr-gui/ui/src/task20.test.ts`: Node-testable panel contracts and pure helpers.
- Modify `crates/aliasmgr-gui/ui/package.json` only if the existing test script needs a new test file glob.
- Modify `plan.md`: Task 20 design/plan/evidence subordinate checkboxes only after CI.

---

### Task 1: Synchronize Task 20 design and plan checkboxes

**Files:**
- Modify `plan.md`
- Modify `docs/superpowers/plans/2026-08-15-task20-gui-diagnostics-settings.md`

- [ ] **Step 1: Mark approved design and planning items**

Under Task 20 in `plan.md`, add Chinese subordinate items:

```markdown
- [x] 设计已确认：core-backed 全真实数据流、分级确认、Doctor 混合分组、Import 单页预览确认、有限 Settings、Retain/Purge 二次确认。
- [x] 实现计划已创建：`docs/superpowers/plans/2026-08-15-task20-gui-diagnostics-settings.md`，当前 agent inline 执行，不启用子代理。
- [ ] Task 20 原始 broad checkbox 保持未完成，直到代码、CI 和 GUI/manual boundary 均有证据。
```

In the Task 20 implementation plan, mark only the design-approved planning entry complete; leave implementation steps unchecked until source and CI evidence exist.

- [ ] **Step 2: Commit checkbox synchronization**

```bash
git add plan.md docs/superpowers/plans/2026-08-15-task20-gui-diagnostics-settings.md
git commit -m "docs: sync task20 design plan checkboxes"
git push origin feature/alias-manager-mvp
```

---

### Task 2: Add core-backed Task 20 command contracts

**Files:**
- Modify `crates/aliasmgr-gui/src-tauri/src/commands.rs`
- Modify `crates/aliasmgr-gui/src-tauri/src/main.rs`
- Create/modify `crates/aliasmgr-gui/ui/src/task20.test.ts`

- [ ] **Step 1: Write failing command contract tests**

Add serializable response types and frontend-facing command names. Tests must assert JSON fields without starting Tauri:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorFindingDto {
    pub shell: Option<String>,
    pub severity: String,
    pub code: String,
    pub message: String,
    pub action: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportPreviewDto {
    pub imported: Vec<String>,
    pub skipped: Vec<String>,
    pub warnings: Vec<String>,
    pub unsupported: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsDto {
    pub config_directory: String,
    pub default_shell: String,
    pub backup_keep: usize,
    pub log_keep: usize,
    pub allow_relative_paths: bool,
}
```

Create Node tests that assert the UI source references these command names:

```ts
import test from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

const source = await readFile(new URL("./App.tsx", import.meta.url), "utf8");

test("Task 20 commands are represented by the GUI shell", () => {
  for (const name of ["doctor_status", "generated_preview", "reload_command", "import_preview", "import_confirm", "config_get", "config_save", "uninstall_preview", "uninstall_confirm", "overridden_definitions"]) {
    assert.match(source, new RegExp(name));
  }
});
```

- [ ] **Step 2: Implement read-only and confirmation command adapters**

Add these Tauri commands, each returning `Result<..., String>` and delegating to core:

```rust
#[tauri::command] pub fn doctor_status() -> Result<Vec<DoctorFindingDto>, String>;
#[tauri::command] pub fn generated_preview(shell: String) -> Result<String, String>;
#[tauri::command] pub fn reload_command(shell: String) -> Result<String, String>;
#[tauri::command] pub fn import_preview(file: String) -> Result<ImportPreviewDto, String>;
#[tauri::command] pub fn import_confirm(file: String) -> Result<ImportPreviewDto, String>;
#[tauri::command] pub fn config_get() -> Result<SettingsDto, String>;
#[tauri::command] pub fn config_save(settings: SettingsDto) -> Result<SettingsDto, String>;
#[tauri::command] pub fn uninstall_preview(purge: bool) -> Result<Vec<String>, String>;
#[tauri::command] pub fn uninstall_confirm(purge: bool) -> Result<Vec<String>, String>;
#[tauri::command] pub fn overridden_definitions() -> Result<Vec<serde_json::Value>, String>;
```

`import_preview` must not write. `import_confirm` may persist only after explicit UI confirmation and must reuse `AliasService::create/update`; no shell command execution is allowed. `uninstall_confirm(true)` must call core PurgeAliases and retain target protection. `config_save` may change only approved limited Settings fields; configuration directory remains read-only.

- [ ] **Step 3: Register commands and commit**

Register every command alongside Task 18/19 commands in `tauri::generate_handler!`:

```rust
commands::doctor_status,
commands::generated_preview,
commands::reload_command,
commands::import_preview,
commands::import_confirm,
commands::config_get,
commands::config_save,
commands::uninstall_preview,
commands::uninstall_confirm,
commands::overridden_definitions,
```

Commit:

```bash
git add crates/aliasmgr-gui/src-tauri/src/commands.rs crates/aliasmgr-gui/src-tauri/src/main.rs crates/aliasmgr-gui/ui/src/task20.test.ts
git commit -m "feat: add task20 gui command contracts"
git push origin feature/alias-manager-mvp
```

---

### Task 3: Implement DoctorPanel and override display

**Files:**
- Create `crates/aliasmgr-gui/ui/src/components/DoctorPanel.tsx`
- Modify `crates/aliasmgr-gui/ui/src/App.tsx`
- Modify `crates/aliasmgr-gui/ui/src/lib.ts`
- Modify `crates/aliasmgr-gui/ui/src/task20.test.ts`

- [ ] **Step 1: Write failing DoctorPanel tests**

Add pure helper tests:

```ts
import { summarizeFindings, canAutoRefreshGenerated } from "./lib";

test("doctor summary groups findings by severity", () => {
  assert.deepEqual(summarizeFindings([
    { severity: "ok" }, { severity: "warning" }, { severity: "failed" }, { severity: "manual" },
  ]), { ok: 1, warning: 1, failed: 1, manual: 1 });
});

test("doctor only exposes safe generated refresh", () => {
  assert.equal(canAutoRefreshGenerated("generated_checksum"), true);
  assert.equal(canAutoRefreshGenerated("profile_order"), false);
  assert.equal(canAutoRefreshGenerated("execution_policy"), false);
});
```

- [ ] **Step 2: Implement DoctorPanel**

DoctorPanel must:

- invoke `doctor_status` on open;
- show severity counts for ok/warning/failed/manual;
- render expandable Shell groups;
- offer `generated_preview`, `reload_command`, and safe refresh only;
- expose copy actions for reload and recovery text;
- never offer Profile/RC reorder, ExecutionPolicy, or user-definition auto-fix;
- render `overridden_definitions` as read-only with recoverable copy only;
- show `recoverable = 0` as not automatically recoverable.

- [ ] **Step 3: Integrate Doctor route and commit**

Replace the Task 19 Doctor placeholder with DoctorPanel while preserving the sidebar and status drawer. Commit:

```bash
git add crates/aliasmgr-gui/ui/src/components/DoctorPanel.tsx crates/aliasmgr-gui/ui/src/App.tsx crates/aliasmgr-gui/ui/src/lib.ts crates/aliasmgr-gui/ui/src/task20.test.ts
git commit -m "feat: add gui doctor panel"
git push origin feature/alias-manager-mvp
```

---

### Task 4: Implement ImportPanel and graded confirmation

**Files:**
- Create `crates/aliasmgr-gui/ui/src/components/ImportPanel.tsx`
- Modify `crates/aliasmgr-gui/ui/src/App.tsx`
- Modify `crates/aliasmgr-gui/ui/src/lib.ts`
- Modify `crates/aliasmgr-gui/ui/src/task20.test.ts`

- [ ] **Step 1: Write failing import flow tests**

```ts
test("import preview does not confirm persistence", () => {
  const state = createImportState();
  assert.equal(state.confirmed, false);
  assert.equal(state.previewed, false);
});

test("purge confirmation is separate from preview", () => {
  assert.equal(canExecutePurge({ selected: true, confirmed: false }), false);
  assert.equal(canExecutePurge({ selected: true, confirmed: true }), true);
});
```

- [ ] **Step 2: Implement single-page ImportPanel**

ImportPanel must:

- choose or receive a file path;
- invoke `import_preview` and show imported/skipped/warnings/unsupported;
- display sensitive-variable, relative-path, unsupported, and conflict warnings;
- keep preview read-only;
- invoke `import_confirm` only from the bottom confirmation action;
- preserve report/draft on failure;
- refresh alias list/tag counts after success;
- never execute imported Shell commands.

- [ ] **Step 3: Integrate import access and commit**

Expose ImportPanel from the appropriate existing shell entry without adding a new navigation architecture. Commit:

```bash
git add crates/aliasmgr-gui/ui/src/components/ImportPanel.tsx crates/aliasmgr-gui/ui/src/App.tsx crates/aliasmgr-gui/ui/src/lib.ts crates/aliasmgr-gui/ui/src/task20.test.ts
git commit -m "feat: add gui import preview confirmation"
git push origin feature/alias-manager-mvp
```

---

### Task 5: Implement limited SettingsPanel

**Files:**
- Create `crates/aliasmgr-gui/ui/src/components/SettingsPanel.tsx`
- Modify `crates/aliasmgr-gui/ui/src/App.tsx`
- Modify `crates/aliasmgr-gui/ui/src/lib.ts`
- Modify `crates/aliasmgr-gui/ui/src/task20.test.ts`

- [ ] **Step 1: Write failing settings draft tests**

```ts
test("settings draft keeps config directory read-only", () => {
  const draft = editableSettings({ config_directory: "/tmp/config", default_shell: "bash", backup_keep: 10, log_keep: 7, allow_relative_paths: false });
  assert.equal(draft.config_directory, "/tmp/config");
  assert.equal(Object.prototype.hasOwnProperty.call(draft, "set_config_directory"), false);
});

test("settings save failure retains draft", () => {
  const result = applySettingsSave({ default_shell: "bash", backup_keep: 10, log_keep: 7, allow_relative_paths: false }, new Error("save failed"));
  assert.equal(result.draft.default_shell, "bash");
  assert.match(result.error, /save failed/);
});
```

- [ ] **Step 2: Implement SettingsPanel**

SettingsPanel loads `config_get`, creates a local draft, and exposes one Save action calling `config_save`. Editable fields are default Shell, backup keep, log keep, and relative-path enablement. Config directory is read-only. Save failure preserves draft and shows a non-sensitive summary. Profile/RC and ExecutionPolicy are informational only.

- [ ] **Step 3: Integrate Settings route and commit**

Replace the Task 19 Settings placeholder with SettingsPanel. Commit:

```bash
git add crates/aliasmgr-gui/ui/src/components/SettingsPanel.tsx crates/aliasmgr-gui/ui/src/App.tsx crates/aliasmgr-gui/ui/src/lib.ts crates/aliasmgr-gui/ui/src/task20.test.ts
git commit -m "feat: add gui limited settings panel"
git push origin feature/alias-manager-mvp
```

---

### Task 6: Implement UninstallPanel and Retain/Purge confirmation

**Files:**
- Create `crates/aliasmgr-gui/ui/src/components/UninstallPanel.tsx`
- Modify `crates/aliasmgr-gui/ui/src/App.tsx`
- Modify `crates/aliasmgr-gui/ui/src/lib.ts`
- Modify `crates/aliasmgr-gui/ui/src/task20.test.ts`

- [ ] **Step 1: Write failing uninstall tests**

```ts
test("purge requires a second confirmation", () => {
  assert.equal(canExecutePurge({ selected: true, confirmed: false }), false);
  assert.equal(canExecutePurge({ selected: true, confirmed: true }), true);
});

test("retain and purge summaries name protected targets", () => {
  assert.match(uninstallSummary(false), /retain/i);
  assert.match(uninstallSummary(true), /target files.*protected/i);
});
```

- [ ] **Step 2: Implement UninstallPanel**

The panel uses Retain/Purge radio selection. Retain explains that generated definitions/targets stay while loader is removed. Purge lists generated/database/journal removal and protected targets. Purge opens a second confirmation before calling `uninstall_confirm(true)`. Both modes show structured results; no target path is passed to a delete operation.

- [ ] **Step 3: Integrate the panel and commit**

Expose UninstallPanel from Settings or the existing lifecycle entry without adding unrelated navigation. Commit:

```bash
git add crates/aliasmgr-gui/ui/src/components/UninstallPanel.tsx crates/aliasmgr-gui/ui/src/App.tsx crates/aliasmgr-gui/ui/src/lib.ts crates/aliasmgr-gui/ui/src/task20.test.ts
git commit -m "feat: add gui uninstall confirmation panel"
git push origin feature/alias-manager-mvp
```

---

### Task 7: Verify Task 20 and update evidence checkboxes

**Files:**
- Modify `plan.md`
- Modify `docs/superpowers/plans/2026-08-15-task20-gui-diagnostics-settings.md`
- Modify `.github/workflows/ci.yml` only if the existing GUI jobs need a focused test command.

- [ ] **Step 1: Run fresh Fast CI**

Use `gh` first; if unavailable, use the absolute path:

```bash
gh workflow run ci.yml --repo yorelll/alias-manage --ref feature/alias-manager-mvp
"D:/Program Files/GitHub CLI/gh.exe" run watch <run-id> --exit-status
```

Expected: Linux/Windows core tests, lint, and GUI frontend test/typecheck/build all pass.

- [ ] **Step 2: Run fresh Integration CI**

```bash
gh workflow run integration.yml --repo yorelll/alias-manage --ref feature/alias-manager-mvp
"D:/Program Files/GitHub CLI/gh.exe" run watch <run-id> --exit-status
```

Expected: existing Bash/Zsh/PowerShell/lock/CLI/transfer/uninstall/isolation jobs all pass.

- [ ] **Step 3: Update Task 20 plan evidence only after CI success**

Add Chinese subordinate evidence under Task 20:

```markdown
- [x] DoctorPanel、ImportPanel、SettingsPanel、UninstallPanel frontend tests/typecheck/build 已通过 Linux/Windows GUI jobs `<run-id>`。
- [x] Task 20 core/command tests已通过 Fast CI `<run-id>`。
- [x] graded confirmation、import preview/write separation、Purge second confirmation、target protection tests已验证。
- [ ] 真实 GUI 窗口、视觉交互、Profile/ExecutionPolicy、file picker、MSI/package hook 人工验收仍待 `docs/gui/`。
- [ ] Task 20 原始 broad checkbox保持未完成，直到所有 required/manual boundary evidence 完成。
```

Update this implementation plan’s completed step markers with actual run IDs. Do not mark Tauri standalone compile or real-window acceptance unless independently verified.

- [ ] **Step 4: Commit final Task 20 evidence**

```bash
git add plan.md docs/superpowers/plans/2026-08-15-task20-gui-diagnostics-settings.md
git commit -m "docs: record task20 gui evidence"
git push origin feature/alias-manager-mvp
```

---

## Plan self-review

- **Coverage:** Doctor, import, settings, uninstall, override display, command registration, confirmation rules, tests, CI, and manual boundaries each have a task.
- **Scope:** No automatic profile/policy/user-definition changes, no MSI/package work, no visual acceptance, and no unrelated Task 21 implementation.
- **Type consistency:** DTO/command names are used consistently between plan, Tauri, and React; `config_get/config_save`, `import_preview/import_confirm`, and `uninstall_preview/uninstall_confirm` form paired flows.
- **Evidence:** No implementation checkbox closes before fresh CI IDs are recorded.
- **Workspace:** GUI remains outside root Cargo workspace and all Rust verification remains CI-only.
