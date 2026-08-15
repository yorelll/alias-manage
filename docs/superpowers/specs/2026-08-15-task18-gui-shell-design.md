# Task 18 GUI Shell Design

## Goal

Initialize the Alias Manager Tauri GUI shell with a persistent sidebar, a list-first landing page, and a collapsible startup status drawer backed by a thin Rust command that reuses `aliasmgr-core`.

## Scope

Task 18 includes the GUI shell, static aliases-page placeholder, startup status command, frontend tests/typecheck, and CI build/test wiring where the existing scaffold supports it.

Task 18 does not implement alias CRUD, full doctor diagnostics, synchronization, import/export, uninstall workflows, visual acceptance, real window startup acceptance, installers, or real-user profile compatibility. Those belong to later tasks or manual GUI validation.

## Architecture

The GUI crate remains outside the root Rust workspace until Tauri dependencies and platform build jobs are intentionally enabled. The UI invokes `startup_status`; `src-tauri/src/commands.rs` is a thin adapter and delegates version, Shell detection, and configuration-path discovery to `aliasmgr-core`.

The response is a serializable `StartupStatus` containing:

- `version`
- `detected_shell`
- `detection_source`
- `config_directory`

The UI must keep the static shell visible if the command fails and show a non-sensitive “status unavailable” state. Task 18 must not create or modify the user configuration directory merely to render startup status.

## UI structure

The approved shell uses:

- Persistent left sidebar: `Aliases`, `Sync`, `Doctor`, `Settings`.
- List-first main page: aliases list placeholder and empty-state/add-alias affordance.
- Collapsible startup status drawer: compact by default; expanded view shows application version, detected Shell, and configuration directory.

The page is a shell only. Sidebar destinations other than the aliases placeholder may remain non-functional placeholders until their tasks are implemented.

## Testing

Rust tests cover the startup response and delegation boundary without duplicating core detection logic. Frontend tests/typecheck cover title, sidebar labels, collapsed/expanded drawer behavior, status rendering, and error fallback.

CI GUI jobs, when enabled, may run frontend tests/typecheck and Tauri compilation. `cargo tauri dev` is never run locally or as a CI requirement for this task. Compilation does not prove visual quality, real-window startup, or user-profile compatibility.

## Manual boundary

Manual GUI validation follows `docs/gui/`: launch, version display, sidebar navigation shell, status drawer expansion, clean configuration behavior, and platform-specific evidence. Visual/interaction acceptance remains separate from CI.

## Error and security rules

Startup errors are represented as structured, non-sensitive responses. The UI must not dump environment variables, complete profiles, tokens, or secret values. The configuration path is displayed as status information only; Task 18 does not automatically write profiles or configuration files.
