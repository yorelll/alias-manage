# Task 19 GUI List, Search, Tags, and Wizard Design

## Goal

Extend the approved Task 18 GUI shell with a real core-backed alias list, text/fuzzy search, AND tag facets, description/tag presentation, and a basic/advanced add/edit wizard with safe persistence.

## Scope

Task 19 includes:

- real SQLite-backed alias listing;
- text and fuzzy search with `limit`;
- repeatable tag facet selection using AND intersection;
- stable alias table presentation for name, target, Shell, description, tags, revision, and status fields available from core;
- description truncation with full-text tooltip;
- basic/advanced wizard sections;
- validation, preview, create, and update flows;
- direct rejection of exact-name and PowerShell case-fold conflicts;
- core service tests and frontend tests/typecheck/build in existing GUI CI.

Task 19 does not include delete/uninstall lifecycle, full doctor, synchronization controls, import/export, package integration, installers, or real-window/visual manual acceptance. Those remain later tasks or manual GUI validation.

## Approved decisions

- **Data flow:** real core-backed data flow, not frontend mock persistence.
- **Core boundary:** add a focused `aliasmgr-core::service` layer. Tauri commands adapt DTOs and delegate to the service; React does not implement SQLite, validation, or filtering rules.
- **Persistence:** Task 19 supports create and update. Delete remains outside this task.
- **Wizard:** basic/advanced two-level form. Basic fields are name, description, target type, executable/script path, Shell, and tags. Advanced fields are fixed arguments, `{{args}}` position, working directory, environment, relative-path option, and argument pass-through.
- **Errors:** field-level inline errors plus a top-level summary, with focus on the first invalid field and draft preservation. Exact-name and PowerShell case-fold conflicts are rejected directly; no overwrite path is offered.
- **GUI shell:** retain Task 18 Persistent sidebar, list-first aliases page, A2 status strip behavior, and B3 expandable status drawer.

## Architecture

```text
React UI
  ├─ AliasTable
  ├─ SearchBar
  ├─ TagFacet
  ├─ TagInput
  └─ AliasWizard
        ↓ invoke()
Tauri commands
        ↓ DTO conversion only
aliasmgr-core::service::AliasService
        ↓
validation / search / storage
        ↓
SQLite
```

The service API is intentionally small:

```rust
pub struct AliasService<'a> {
    pub database: &'a Database,
}

impl<'a> AliasService<'a> {
    pub fn list(&self) -> Result<Vec<AliasRecord>, AliasError>;
    pub fn search(&self, query: &SearchQuery) -> Result<Vec<SearchResult>, AliasError>;
    pub fn tag_counts(&self) -> Result<BTreeMap<String, usize>, AliasError>;
    pub fn create(&self, alias: AliasRecord) -> Result<AliasRecord, AliasError>;
    pub fn update(&self, alias: AliasRecord) -> Result<AliasRecord, AliasError>;
}
```

The service delegates to existing core functions. It computes a record checksum before create/update, calls validation, and lets storage return `ExactNameConflict`, `CaseFoldConflict`, `ChecksumMismatch`, and other existing errors. It does not introduce GUI/Tauri types into core.

## UI behavior

### Alias table

The table shows name, target, Shell, description, tags, revision, enabled state, and available per-Shell state. Long descriptions are visually truncated while the complete value remains available via a tooltip/title. Tags use stable chip styling.

### Search and facets

`SearchBar` sends query, fuzzy flag, and limit to `search_aliases`. `TagFacet` obtains counts through `tag_counts` and sends selected tags as `SearchQuery.tag_filter`. Multiple tags are ANDed in core. Clearing all tags restores the unfiltered query. Text query and tag filters are combined in one service request; the frontend does not filter a full result set locally.

### Wizard

The wizard exposes basic fields first and expands advanced fields explicitly. It supports preview before save. On validation or conflict failure, the top summary and field-level messages are shown, the first invalid field receives focus, and the current draft remains intact. Successful create/update refreshes the list through the service-backed command.

## Tauri commands and DTOs

Task 19 adds thin commands:

- `list_aliases`
- `search_aliases`
- `tag_counts`
- `create_alias`
- `update_alias`

The UI receives serializable alias views containing id, name, description, executable, target type, fixed args, pass-through, working directory, environment, Shells, enabled, tags, and revision. Tauri error responses are structured and non-sensitive.

## Testing and CI

Core tests cover real in-memory SQLite service behavior, search/limit/tag AND semantics, tag counts, create/update validation, checksum recalculation, exact-name conflicts, PowerShell case-fold conflicts, and missing records.

Frontend Node tests cover table fields/truncation, SearchBar command payloads, TagFacet counts and AND selection, clear-filter behavior, wizard open/basic-advanced fields, preview, inline/top-level validation errors, conflict draft preservation, and successful list refresh. Existing Linux/Windows GUI frontend jobs run npm test, typecheck, and build. Existing fast CI runs core service/search tests. No local Cargo/Rust/Shell/Tauri commands are used.

## Manual boundary

Manual GUI validation remains responsible for Linux/Windows window startup, visual layout, real file picker behavior, user-profile compatibility, PowerShell policy, and interaction quality. Evidence is recorded using `docs/gui/` and must not be inferred from frontend CI success.
