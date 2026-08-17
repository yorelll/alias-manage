# Task 19 GUI List, Search, Tags, and Wizard Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a real core-backed alias list, search/tag facets, and safe add/edit wizard to the approved Task 18 GUI shell.

**Architecture:** Add a focused `aliasmgr-core::service::AliasService` that delegates validation, search, checksum, and storage operations. Tauri commands convert serializable DTOs and call the service; React components render table/search/facet/wizard state and never implement SQLite or filtering rules. The GUI crate remains outside the root Cargo workspace.

**Tech Stack:** Rust, `aliasmgr-core`, rusqlite, Serde, Tauri 2 commands, React/TypeScript, Node test, Vite, GitHub Actions.

---

## Scope and verification boundary

- Task 19 supports list/search/tag counts/create/update/preview only.
- Delete, uninstall, full doctor, sync controls, import/export, installers, and package hooks remain outside this task.
- Do not run local Cargo, Rust, Shell, Tauri, or GUI dev-server commands. All Rust and frontend checks run in GitHub Actions.
- Keep `crates/aliasmgr-gui` outside root `Cargo.toml` workspace members.
- Use existing GUI frontend CI jobs for npm test/typecheck/build and existing fast CI for core tests.
- Keep original Task 19 broad checkbox unchecked until all required implementation and evidence are complete; preserve real-window/visual/manual acceptance as separate unchecked items.

## File map

- Create `crates/aliasmgr-core/src/service.rs`: core alias query/mutation service.
- Modify `crates/aliasmgr-core/src/lib.rs`: export `service`.
- Create `crates/aliasmgr-core/src/service.rs` tests: real in-memory Database service behavior.
- Modify `crates/aliasmgr-gui/src-tauri/src/commands.rs`: serializable DTOs and thin list/search/tag/create/update commands.
- Modify `crates/aliasmgr-gui/src-tauri/src/main.rs`: register Task 19 commands alongside `startup_status`.
- Create `crates/aliasmgr-gui/ui/src/components/AliasTable.tsx`: table rendering and description/tag/status presentation.
- Create `crates/aliasmgr-gui/ui/src/components/SearchBar.tsx`: query/fuzzy/limit controls.
- Create `crates/aliasmgr-gui/ui/src/components/TagInput.tsx`: chip entry, trim, deduplicate, and draft-level normalization.
- Create `crates/aliasmgr-gui/ui/src/components/TagFacet.tsx`: counted multi-select AND facet.
- Create `crates/aliasmgr-gui/ui/src/components/AliasWizard.tsx`: basic/advanced create/edit form, preview, validation display, and save callbacks.
- Modify `crates/aliasmgr-gui/ui/src/App.tsx`: service-backed page state, list refresh, search/facet state, wizard open/edit state.
- Modify `crates/aliasmgr-gui/ui/src/lib.ts`: Task 19 DTOs, command adapter helpers, and pure UI helpers.
- Modify `crates/aliasmgr-gui/ui/src/app.test.ts`: pure frontend behavior tests.
- Create `crates/aliasmgr-gui/ui/src/task19.test.ts`: table/search/facet/wizard source and helper tests.
- Modify `crates/aliasmgr-gui/ui/package.json` only if a test script/dependency change is required.
- Modify `plan.md`: Task 19 evidence checkboxes only after CI success.

---

### Task 1: Add the core AliasService query boundary

**Files:**
- Create: `crates/aliasmgr-core/src/service.rs`
- Modify: `crates/aliasmgr-core/src/lib.rs`

- [x] **Design approved:** real core-backed data flow, `AliasService` boundary, Add/Edit+Save scope, basic/advanced wizard, and direct conflict rejection are documented in `docs/superpowers/specs/2026-08-15-task19-gui-list-search-wizard-design.md`.

- [x] **Design approved:** real core-backed data flow, `AliasService` boundary, Add/Edit+Save scope, basic/advanced wizard, and direct conflict rejection are documented in `docs/superpowers/specs/2026-08-15-task19-gui-list-search-wizard-design.md`.

- [ ] **Step 1: Write failing service tests**

Create tests using `Database::open_in_memory()` and real `AliasRecord` values. The tests must express the service API before implementation:

```rust
#[test]
fn service_lists_searches_and_counts_tags() {
    let database = Database::open_in_memory().unwrap();
    let mut first = AliasRecord { name: "gs".into(), executable: "git".into(), ..Default::default() };
    first.tags = vec!["git".into(), "work".into()];
    first.record_checksum = record_checksum(&first).unwrap();
    database.insert_alias(&first).unwrap();

    let service = AliasService::new(&database);
    assert_eq!(service.list().unwrap()[0].name, "gs");
    let query = SearchQuery { query: "gs".into(), tag_filter: vec!["git".into(), "work".into()], ..Default::default() };
    assert_eq!(service.search(&query).unwrap()[0].alias.name, "gs");
    assert_eq!(service.tag_counts().unwrap().get("git"), Some(&1));
}

#[test]
fn service_create_rejects_invalid_and_conflicting_records() {
    let database = Database::open_in_memory().unwrap();
    let service = AliasService::new(&database);
    let invalid = AliasRecord { name: "bad name".into(), executable: "tool".into(), ..Default::default() };
    assert!(matches!(service.create(invalid), Err(AliasError::InvalidAliasName)));
}

#[test]
fn service_update_recalculates_checksum_and_rejects_name_conflict() {
    let database = Database::open_in_memory().unwrap();
    let service = AliasService::new(&database);
    let mut alias = AliasRecord { name: "one".into(), executable: "tool".into(), ..Default::default() };
    alias.record_checksum = record_checksum(&alias).unwrap();
    database.insert_alias(&alias).unwrap();
    alias.description = "updated".into();
    let saved = service.update(alias).unwrap();
    assert_eq!(saved.record_checksum, record_checksum(&saved).unwrap());
}
```

- [ ] **Step 2: Add the minimal service implementation**

Implement `service.rs` with these signatures:

```rust
use std::collections::BTreeMap;
use crate::{error::AliasError, model::{record_checksum, AliasRecord}, search::{search, SearchQuery, SearchResult}, storage::Database, validation::validate_alias};

pub struct AliasService<'a> { pub database: &'a Database }

impl<'a> AliasService<'a> {
    pub fn new(database: &'a Database) -> Self { Self { database } }
    pub fn list(&self) -> Result<Vec<AliasRecord>, AliasError> { self.database.list_aliases() }
    pub fn search(&self, query: &SearchQuery) -> Result<Vec<SearchResult>, AliasError> { Ok(search(&self.database.list_aliases()?, query)) }
    pub fn tag_counts(&self) -> Result<BTreeMap<String, usize>, AliasError> {
        let mut counts = BTreeMap::new();
        for alias in self.database.list_aliases()? {
            for tag in alias.tags { *counts.entry(tag).or_insert(0) += 1; }
        }
        Ok(counts)
    }
    pub fn create(&self, mut alias: AliasRecord) -> Result<AliasRecord, AliasError> {
        validate_alias(&alias)?;
        alias.record_checksum = record_checksum(&alias)?;
        self.database.insert_alias(&alias)?;
        Ok(alias)
    }
    pub fn update(&self, mut alias: AliasRecord) -> Result<AliasRecord, AliasError> {
        validate_alias(&alias)?;
        alias.record_checksum = record_checksum(&alias)?;
        if !self.database.update_alias(&alias)? { return Err(AliasError::Config("alias not found".into())); }
        Ok(alias)
    }
}
```

The service must not import Tauri or frontend types. Existing storage conflict and checksum behavior remains authoritative.

- [x] Task 1 core service slice implemented in `8b97c09`; fast CI `31991047603` and integration `31991181943` passed.

- [ ] **Step 3: Export the service and commit**

Add `pub mod service;` to `crates/aliasmgr-core/src/lib.rs`. Inspect that `SearchQuery` already includes `tag_filter`; do not duplicate it. Commit only the core service slice:

```bash
git add crates/aliasmgr-core/src/service.rs crates/aliasmgr-core/src/lib.rs
git commit -m "feat: add core alias service boundary"
git push origin feature/alias-manager-mvp
```

Verify through fast CI; do not run local Cargo.

---

### Task 2: Add Tauri DTOs and core-backed commands

**Files:**
- Modify: `crates/aliasmgr-gui/src-tauri/src/commands.rs`
- Modify: `crates/aliasmgr-gui/src-tauri/src/main.rs`

- [ ] **Step 1: Write command contract tests**

Add serializable DTOs and tests for conversion without starting Tauri:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AliasDto { pub id: String, pub name: String, pub description: String, pub executable: String, pub target_type: String, pub fixed_args: Vec<String>, pub pass_args: bool, pub working_directory: Option<String>, pub environment: BTreeMap<String, String>, pub shells: Vec<String>, pub enabled: bool, pub tags: Vec<String>, pub revision: i64 }

#[test]
fn alias_dto_serializes_required_list_fields() {
    let dto = alias_to_dto(&AliasRecord::default());
    let json = serde_json::to_string(&dto).unwrap();
    assert!(json.contains("name"));
    assert!(json.contains("tags"));
    assert!(json.contains("revision"));
}
```

- [ ] **Step 2: Implement commands with isolated database resolution**

Add thin commands:

```rust
#[tauri::command]
pub fn list_aliases() -> Result<Vec<AliasDto>, String>;

#[tauri::command]
pub fn search_aliases(query: SearchRequest) -> Result<Vec<AliasDto>, String>;

#[tauri::command]
pub fn tag_counts() -> Result<BTreeMap<String, usize>, String>;

#[tauri::command]
pub fn create_alias(alias: AliasDto) -> Result<AliasDto, String>;

#[tauri::command]
pub fn update_alias(alias: AliasDto) -> Result<AliasDto, String>;
```

`SearchRequest` contains `query`, `fuzzy`, `limit`, and `tag_filter`. Commands resolve the configured database path using `AppPaths::discover(None)`/the existing config boundary, construct `AliasService`, and convert `AliasError` to non-sensitive strings. Do not put validation, search scoring, or SQLite SQL in commands.

- [x] Task 2 GUI commands implemented in `8b97c09`; command registration and DTO conversion are included in the same slice.

- [ ] **Step 3: Register commands and commit**

> The planned standalone command commit step is represented by `8b97c09`; remaining plan checkboxes below track implementation evidence, not a second duplicate commit.

Register all five commands in `tauri::generate_handler!` next to `startup_status`:

```rust
.invoke_handler(tauri::generate_handler![
    commands::startup_status,
    commands::list_aliases,
    commands::search_aliases,
    commands::tag_counts,
    commands::create_alias,
    commands::update_alias,
])
```

Commit:

```bash
git add crates/aliasmgr-gui/src-tauri/src/commands.rs crates/aliasmgr-gui/src-tauri/src/main.rs
git commit -m "feat: add gui alias service commands"
git push origin feature/alias-manager-mvp
```

---

### Task 3: Implement AliasTable, SearchBar, and TagFacet

**Files:**
- Create: `crates/aliasmgr-gui/ui/src/components/AliasTable.tsx`
- Create: `crates/aliasmgr-gui/ui/src/components/SearchBar.tsx`
- Create: `crates/aliasmgr-gui/ui/src/components/TagFacet.tsx`
- Modify: `crates/aliasmgr-gui/ui/src/lib.ts`
- Modify: `crates/aliasmgr-gui/ui/src/App.tsx`
- Create: `crates/aliasmgr-gui/ui/src/task19.test.ts`

- [ ] **Step 1: Write frontend tests first**

Create tests against pure helpers and source contracts:

```ts
import test from "node:test";
import assert from "node:assert/strict";
import { normalizeTags, applyTagSelection, truncateDescription } from "./lib";

test("normalizes tags by trim, case-preserving deduplication, and empty removal", () => {
  assert.deepEqual(normalizeTags([" git ", "git", "", "work"]), ["git", "work"]);
});

test("tag selections use AND intersection", () => {
  assert.deepEqual(applyTagSelection(["git", "work"], ["git", "work"]), ["git", "work"]);
  assert.deepEqual(applyTagSelection(["git"], ["git", "work"]), ["git"]);
});

test("description truncation preserves full tooltip value", () => {
  const result = truncateDescription("a".repeat(100), 40);
  assert.equal(result.full.length, 100);
  assert.equal(result.display.length, 40);
  assert.equal(result.truncated, true);
});
```

- [ ] **Step 2: Implement the pure UI helpers**

Add to `ui/src/lib.ts`:

```ts
export function normalizeTags(values: string[]): string[] {
  return [...new Set(values.map((value) => value.trim()).filter(Boolean))];
}

export function applyTagSelection(current: string[], selected: string[]): string[] {
  return current.filter((tag) => selected.includes(tag));
}

export function truncateDescription(full: string, max: number): { display: string; full: string; truncated: boolean } {
  return full.length <= max ? { display: full, full, truncated: false } : { display: full.slice(0, max), full, truncated: true };
}
```

- [x] Task 3 frontend list/search/tag components implemented in `cf1cdea`; frontend CI evidence is recorded below.

- [ ] **Step 3: Implement the components**

> The implementation is already included in `cf1cdea`; this checklist remains as the task-level source steps.

`AliasTable` receives `AliasDto[]`, renders name/target/Shell/description/tags/revision/enabled, uses `title={description}` for truncated descriptions, and uses stable chip classes based on tag order. It must not issue database calls.

`SearchBar` receives `query`, `fuzzy`, `limit`, and `onChange`, and emits one `SearchRequest` payload to the parent.

`TagFacet` receives `Record<string, number>`, selected tags, and `onChange`; clicking toggles tags, and the parent sends all selected tags to `search_aliases` in one request. Clearing selection sends an empty `tag_filter`.

- [ ] **Step 4: Integrate list/search/facet state in App.tsx**

Keep the Task 18 status drawer and sidebar. In the Aliases page:

- load `list_aliases` and `tag_counts` on mount;
- call `search_aliases` when query/fuzzy/limit/tag selection changes;
- show loading/error state without removing the shell;
- render `AliasTable`, `SearchBar`, and `TagFacet`;
- preserve the disabled Add alias button until Task 4 adds the wizard.

- [ ] **Step 5: Commit list/search/facet slice**

```bash
git add crates/aliasmgr-gui/ui/src/components/AliasTable.tsx crates/aliasmgr-gui/ui/src/components/SearchBar.tsx crates/aliasmgr-gui/ui/src/components/TagFacet.tsx crates/aliasmgr-gui/ui/src/lib.ts crates/aliasmgr-gui/ui/src/App.tsx crates/aliasmgr-gui/ui/src/task19.test.ts
git commit -m "feat: add gui alias list search and tag facets"
git push origin feature/alias-manager-mvp
```

---

### Task 4: Implement TagInput and the basic/advanced AliasWizard

**Files:**
- Create: `crates/aliasmgr-gui/ui/src/components/TagInput.tsx`
- Create: `crates/aliasmgr-gui/ui/src/components/AliasWizard.tsx`
- Modify: `crates/aliasmgr-gui/ui/src/App.tsx`
- Modify: `crates/aliasmgr-gui/ui/src/lib.ts`
- Modify: `crates/aliasmgr-gui/ui/src/task19.test.ts`

- [ ] **Step 1: Write wizard tests first**

Add pure draft helpers/tests:

```ts
import { validateDraftName, draftToAliasRequest, normalizeTags } from "./lib";

test("rejects invalid alias names before save", () => {
  assert.equal(validateDraftName("bad name"), "invalid alias name");
  assert.equal(validateDraftName("gs"), null);
});

test("draft conversion keeps advanced structured fields", () => {
  const request = draftToAliasRequest({
    id: null, name: "gs", description: "Git status", target_type: "native_executable", executable: "git",
    fixed_args: ["status"], pass_args: true, working_directory: null, environment: {}, shells: ["bash"], tags: ["git", "work"], advanced: true,
  });
  assert.equal(request.name, "gs");
  assert.deepEqual(request.fixed_args, ["status"]);
  assert.deepEqual(request.tags, ["git", "work"]);
});
```

- [ ] **Step 2: Implement TagInput**

`TagInput` must:

- display tags as removable chips;
- trim on Enter/comma;
- deduplicate using `normalizeTags`;
- offer existing tag suggestions supplied by the parent;
- never save invalid/empty tags;
- keep changes in the draft until save succeeds.

- [x] Task 4 basic/advanced wizard and TagInput implementation included in `cf1cdea`; frontend tests/typecheck/build passed in Fast CI `31991047603`.

- [ ] **Step 3: Implement AliasWizard basic/advanced sections**

> The implementation is already included in `cf1cdea`; this checklist remains as the task-level source steps.

Basic fields: name, description, target type, executable/script path, Shell, tags.

Advanced fields behind an explicit toggle: fixed args, `{{args}}` position, working directory, environment entries, relative path option, pass-through.

The wizard receives `initialAlias?: AliasDto`, `onCancel`, and `onSaved`. It must expose `Preview` and `Save` actions. Preview renders the structured draft, not executable shell evaluation.

On save:

1. Run `validateDraftName` for immediate field feedback.
2. Build the DTO.
3. Invoke `create_alias` or `update_alias`.
4. On `ExactNameConflict`/`CaseFoldConflict`, show top error summary and name-field error, preserve draft, and do not offer overwrite.
5. On other validation errors, map the known core error text to the relevant field when possible, preserve draft, and focus the first error.
6. On success, close the wizard and refresh list/tag counts.

- [ ] **Step 4: Integrate Add/Edit flow**

Enable Add alias only when the wizard is implemented. Add table-row Edit action. Keep Delete absent. Ensure opening the wizard does not mutate SQLite; only Save invokes create/update.

- [ ] **Step 5: Commit the wizard slice**

```bash
git add crates/aliasmgr-gui/ui/src/components/TagInput.tsx crates/aliasmgr-gui/ui/src/components/AliasWizard.tsx crates/aliasmgr-gui/ui/src/App.tsx crates/aliasmgr-gui/ui/src/lib.ts crates/aliasmgr-gui/ui/src/task19.test.ts
git commit -m "feat: add gui alias editor wizard"
git push origin feature/alias-manager-mvp
```

---

### Task 5: Run GUI CI, update Task 19 evidence, and preserve manual boundaries

**Files:**
- Modify: `.github/workflows/ci.yml` only if the existing GUI jobs need a focused Task 19 command.
- Modify: `plan.md`
- Modify: `docs/gui/README.md` only to link the Task 19 manual cases if needed.

- [x] **Step 1: Run fresh fast CI** — `31991047603` passed Linux/Windows core, lint, and GUI frontend jobs.

- [x] **Step 2: Run fresh integration workflow** — `31991181943` passed the full existing integration matrix.

- [x] **Step 3: Update Task 19 checkboxes only with evidence** — `plan.md` records the implemented core/frontend evidence and leaves runtime/manual gaps unchecked.

- [ ] **Step 1: Run fresh fast CI**

Use `gh` first and absolute fallback:

```bash
gh workflow run ci.yml --repo yorelll/alias-manage --ref feature/alias-manager-mvp
"D:/Program Files/GitHub CLI/gh.exe" run watch <run-id> --exit-status
```

Expected: existing lint/test-linux/test-windows and GUI Linux/Windows frontend jobs all succeed. No local Cargo/npm command is allowed.

- [ ] **Step 2: Run fresh integration workflow**

```bash
gh workflow run integration.yml --repo yorelll/alias-manage --ref feature/alias-manager-mvp
"D:/Program Files/GitHub CLI/gh.exe" run watch <run-id> --exit-status
```

Expected: all existing Bash/Zsh/PowerShell/lock/CLI isolation jobs succeed. Task 19 does not claim real GUI window startup from this workflow.

- [ ] **Step 3: Update Task 19 checkboxes only with evidence**

After both workflows pass, add Chinese subordinate evidence such as:

```markdown
- [x] core AliasService 已实现并由 fast CI run `<id>` 验证。
- [x] GUI list/search/tag facet frontend tests、typecheck、build 已由 Linux/Windows GUI jobs run `<id>` 验证。
- [x] create/update wizard validation and conflict rejection tests 已由 GUI/core CI run `<id>` 验证。
- [ ] Linux/Windows 真实窗口启动、视觉布局和交互人工验收仍待 `docs/gui/` 报告。
```

Keep the original Task 19 broad checkbox unchecked until its complete requirements and manual boundaries are satisfied.

- [ ] **Step 4: Commit evidence and plan updates**

```bash
git add plan.md docs/gui/README.md
git commit -m "docs: record task19 gui evidence"
git push origin feature/alias-manager-mvp
```

---

## Plan self-review

- **Spec coverage:** real service/query/mutation boundary is Task 1; DTO commands are Task 2; table/search/facet behavior is Task 3; C basic/advanced wizard and A create/update persistence are Task 4; CI/manual boundary is Task 5.
- **Conflict behavior:** create/update never offers overwrite and preserves drafts on `ExactNameConflict`/`CaseFoldConflict`.
- **Tag behavior:** core owns `SearchQuery.tag_filter` AND semantics; UI only sends selected tags.
- **Workspace boundary:** GUI remains outside root workspace; only existing standalone GUI frontend CI is extended.
- **No placeholders:** every task names files, tests, API shapes, commands, expected CI outcome, or explicit scope boundary.
- **Type consistency:** `AliasDto`, `SearchRequest`, `StartupStatus`, `normalizeTags`, `truncateDescription`, `AliasService`, and command names are reused consistently across tasks.
- **Manual boundary:** no checkbox claims visual acceptance, real window startup, or user profile compatibility from frontend CI.
