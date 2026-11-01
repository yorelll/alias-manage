# Task 18 GUI Shell Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Initialize the Task 18 Tauri GUI shell with the approved persistent sidebar, list-first aliases page, expandable startup status drawer, and a thin core-backed startup command.

**Architecture:** Keep `crates/aliasmgr-gui` outside the root Cargo workspace until GUI CI is explicitly enabled. The Tauri Rust layer owns only application bootstrapping and command registration; `startup_status` delegates version, Shell detection, and configuration-path discovery to `aliasmgr-core`. The TypeScript UI renders a static shell and invokes the startup command through a small adapter, with a non-fatal error state.

**Tech Stack:** Tauri 2, Rust stable, `aliasmgr-core`, TypeScript, React, Node test/typecheck, GitHub Actions on Linux and Windows.

---

## Scope and verification boundary

- Do not add `crates/aliasmgr-gui` to the root `Cargo.toml` workspace members.
- Do not implement alias CRUD, database list/save/delete, full doctor, synchronization, import/export, uninstall, installers, or GUI visual acceptance in Task 18.
- Do not run local `cargo`, `rustc`, `rustup`, Bash, Zsh, PowerShell, Tauri, or GUI dev-server commands.
- Rust/Tauri compilation and frontend checks run only in GitHub Actions.
- `cargo tauri dev` is not a CI or local requirement. A successful compile does not prove window startup or visual acceptance.
- Update `plan.md` Task 18 checkboxes only after successful CI evidence. Keep Linux/Windows startup and visual/manual items unchecked until a user records them using `docs/gui/`.

## File map

- Create `crates/aliasmgr-gui/src-tauri/Cargo.toml`: standalone Tauri Rust package, not a root workspace member.
- Create `crates/aliasmgr-gui/src-tauri/build.rs`: Tauri build-script invocation.
- Create `crates/aliasmgr-gui/src-tauri/src/main.rs`: Tauri builder, command registration, and app entry point.
- Create `crates/aliasmgr-gui/src-tauri/src/commands.rs`: serializable `StartupStatus`, thin core-backed startup command, and command tests.
- Create `crates/aliasmgr-gui/src-tauri/tauri.conf.json`: Tauri window/build configuration pointing at the frontend dist directory.
- Create `crates/aliasmgr-gui/src-tauri/capabilities/default.json`: minimal default capability for the main window.
- Create `crates/aliasmgr-gui/ui/package.json`: frontend scripts and minimal dependencies.
- Create `crates/aliasmgr-gui/ui/tsconfig.json`: strict TypeScript configuration.
- Create `crates/aliasmgr-gui/ui/index.html`: frontend entry document.
- Modify `crates/aliasmgr-gui/ui/src/main.ts`: render the React application.
- Modify `crates/aliasmgr-gui/ui/src/App.tsx`: A2/B3 shell and startup-status state.
- Create `crates/aliasmgr-gui/ui/src/app.test.ts`: Node-testable static source/behavior checks.
- Modify `.github/workflows/ci.yml`: add GUI Linux/Windows frontend and Tauri compile jobs only after the standalone scaffold builds.
- Modify `plan.md`: close only CI-supported Task 18 subordinate items; preserve manual/visual items as unchecked.

---

### Task 1: Make the standalone Tauri package buildable

**Files:**
- Create: `crates/aliasmgr-gui/src-tauri/Cargo.toml`
- Create: `crates/aliasmgr-gui/src-tauri/build.rs`
- Create: `crates/aliasmgr-gui/src-tauri/tauri.conf.json`
- Create: `crates/aliasmgr-gui/src-tauri/capabilities/default.json`
- Modify: `crates/aliasmgr-gui/ui/package.json`
- Create: `crates/aliasmgr-gui/ui/index.html`
- Create: `crates/aliasmgr-gui/ui/tsconfig.json`

- [ ] **Step 1: Write the standalone package configuration**

Create `crates/aliasmgr-gui/src-tauri/Cargo.toml` with a package that is not declared as a workspace member:

```toml
[package]
name = "aliasmgr-gui"
version = "0.1.0"
edition = "2021"

[build-dependencies]
tauri-build = "2"

[dependencies]
aliasmgr-core = { path = "../../aliasmgr-core" }
serde = { version = "1", features = ["derive"] }
tauri = { version = "2", features = [] }
```

Create `build.rs`:

```rust
fn main() {
    tauri_build::build()
}
```

Create `tauri.conf.json` with a single 1100x720 window, frontend dist directory, and no development URL dependency:

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "Alias Manager",
  "version": "0.1.0",
  "identifier": "com.aliasmanager.app",
  "build": {
    "frontendDist": "../ui/dist"
  },
  "app": {
    "windows": [
      {
        "title": "Alias Manager",
        "width": 1100,
        "height": 720,
        "minWidth": 860,
        "minHeight": 560,
        "resizable": true
      }
    ],
    "security": { "csp": null }
  },
  "bundle": { "active": false }
}
```

Create `capabilities/default.json` with only the main-window capability:

```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "Default capability for the Alias Manager window",
  "windows": ["main"],
  "permissions": ["core:default"]
}
```

- [ ] **Step 2: Configure the frontend build and typecheck scripts**

Update `ui/package.json` to use a minimal Vite/React build while keeping Node-only verification available:

```json
{
  "name": "aliasmgr-ui",
  "private": true,
  "type": "module",
  "scripts": {
    "build": "tsc --noEmit && vite build",
    "test": "node --test",
    "typecheck": "tsc --noEmit"
  },
  "dependencies": {
    "@tauri-apps/api": "^2.0.0",
    "react": "^18.3.1",
    "react-dom": "^18.3.1"
  },
  "devDependencies": {
    "@types/react": "^18.3.12",
    "@types/react-dom": "^18.3.1",
    "@vitejs/plugin-react": "^4.3.4",
    "typescript": "^5.7.0",
    "vite": "^6.0.0"
  }
}
```

Create `ui/tsconfig.json`:

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "useDefineForClassFields": true,
    "lib": ["ES2022", "DOM", "DOM.Iterable"],
    "allowJs": false,
    "skipLibCheck": true,
    "esModuleInterop": true,
    "allowSyntheticDefaultImports": true,
    "strict": true,
    "module": "ESNext",
    "moduleResolution": "Bundler",
    "resolveJsonModule": true,
    "isolatedModules": true,
    "noEmit": true,
    "jsx": "react-jsx"
  },
  "include": ["src"]
}
```

Create `ui/index.html`:

```html
<!doctype html>
<html lang="zh-CN">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>Alias Manager</title>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/main.ts"></script>
  </body>
</html>
```

- [ ] **Step 3: Commit the standalone scaffold configuration**

Before implementation continues, inspect that root `Cargo.toml` still excludes `crates/aliasmgr-gui`. Commit only the package/configuration files:

```bash
git add crates/aliasmgr-gui/src-tauri/Cargo.toml crates/aliasmgr-gui/src-tauri/build.rs crates/aliasmgr-gui/src-tauri/tauri.conf.json crates/aliasmgr-gui/src-tauri/capabilities/default.json crates/aliasmgr-gui/ui/package.json crates/aliasmgr-gui/ui/tsconfig.json crates/aliasmgr-gui/ui/index.html
git commit -m "feat: scaffold standalone tauri gui package"
git push origin feature/alias-manager-mvp
```

Verify this commit only in GitHub Actions; do not run local Cargo/npm commands.

---

### Task 2: Add the core-backed startup command

**Files:**
- Create: `crates/aliasmgr-gui/src-tauri/src/commands.rs`
- Modify: `crates/aliasmgr-gui/src-tauri/src/main.rs`

- [ ] **Step 1: Write the failing Rust command tests**

Create `commands.rs` with the target response shape and tests first. The response must serialize and use core APIs rather than duplicate detection logic:

```rust
use aliasmgr_core::{
    config::AppPaths,
    detection::{detect, DetectionContext},
};
use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct StartupStatus {
    pub version: String,
    pub detected_shell: Option<String>,
    pub detection_source: String,
    pub config_directory: String,
}

#[tauri::command]
pub fn startup_status() -> StartupStatus {
    let detection = detect(&DetectionContext {
        explicit: None,
        parent_shell: None,
        shell_env: std::env::var("SHELL").ok(),
        login_shell: None,
    });
    let paths = AppPaths::discover(None);
    StartupStatus {
        version: aliasmgr_core::version().to_string(),
        detected_shell: detection.selected.map(|shell| format!("{shell:?}")),
        detection_source: detection.source,
        config_directory: paths.root.to_string_lossy().into_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_status_uses_core_version_and_serializes() {
        let status = startup_status();
        assert_eq!(status.version, aliasmgr_core::version());
        assert!(!status.detection_source.is_empty());
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("version"));
        assert!(json.contains("config_directory"));
    }
}
```

Expected red result before the command and Tauri package are wired: the command/module or Tauri dependency is unavailable.

- [ ] **Step 2: Wire the command into the Tauri builder**

Create `main.rs`:

```rust
mod commands;

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![commands::startup_status])
        .run(tauri::generate_context!())
        .expect("error while running Alias Manager");
}
```

The command must not create the configuration directory or write profiles. The only side effects allowed are reads performed by core detection/config discovery.

- [ ] **Step 3: Commit the command slice**

```bash
git add crates/aliasmgr-gui/src-tauri/src/main.rs crates/aliasmgr-gui/src-tauri/src/commands.rs
git commit -m "feat: add core-backed gui startup command"
git push origin feature/alias-manager-mvp
```

Verify the standalone Rust package and command test in the GUI-specific CI job, not locally.

---

### Task 3: Implement the approved A2/B3 frontend shell

**Files:**
- Modify: `crates/aliasmgr-gui/ui/src/main.ts`
- Modify: `crates/aliasmgr-gui/ui/src/App.tsx`
- Create: `crates/aliasmgr-gui/ui/src/app.test.ts`
- Create: `crates/aliasmgr-gui/ui/vite.config.ts`

- [ ] **Step 1: Write frontend tests against the required shell contract**

Create `ui/src/app.test.ts` as a source-level Node test that does not require a browser or Tauri runtime:

```js
import test from 'node:test'
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'

const source = await readFile(new URL('./App.tsx', import.meta.url), 'utf8')

test('renders the approved persistent sidebar shell', () => {
  for (const label of ['Aliases', 'Sync', 'Doctor', 'Settings']) {
    assert.match(source, new RegExp(label))
  }
  assert.match(source, /startup_status/)
  assert.match(source, /config_directory/)
})

test('keeps startup status drawer collapsed by default and exposes fallback state', () => {
  assert.match(source, /useState\(false\)/)
  assert.match(source, /status unavailable/i)
})
```

Run only frontend tests in GUI CI; no local npm command is required by this repository policy.

- [ ] **Step 2: Implement the thin Tauri invoke adapter and static shell**

Create `vite.config.ts`:

```ts
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

export default defineConfig({ plugins: [react()] })
```

Create `main.ts`:

```tsx
import React from 'react'
import { createRoot } from 'react-dom/client'
import App from './App'
import './styles.css'

createRoot(document.getElementById('root')!).render(
  <React.StrictMode><App /></React.StrictMode>,
)
```

Implement `App.tsx` with the approved structure. The command failure must not remove the shell:

```tsx
import { useEffect, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'

type StartupStatus = {
  version: string
  detected_shell: string | null
  detection_source: string
  config_directory: string
}

export default function App() {
  const [expanded, setExpanded] = useState(false)
  const [status, setStatus] = useState<StartupStatus | null>(null)
  const [statusError, setStatusError] = useState(false)

  useEffect(() => {
    invoke<StartupStatus>('startup_status')
      .then(setStatus)
      .catch(() => setStatusError(true))
  }, [])

  return (
    <main className="app-shell">
      <aside className="sidebar" aria-label="Primary navigation">
        <h1>Alias Manager</h1>
        <nav>
          <button>Aliases</button>
          <button>Sync</button>
          <button>Doctor</button>
          <button>Settings</button>
        </nav>
      </aside>
      <section className="workspace">
        <button className="status-drawer" onClick={() => setExpanded(value => !value)} aria-expanded={expanded}>
          {status ? `${status.version} · ${status.detected_shell ?? 'Shell unknown'} · config` : 'Startup status'}
        </button>
        {expanded && (
          <section className="status-panel" aria-label="Startup status">
            {statusError && <p>Status unavailable</p>}
            {status && <dl>
              <dt>Version</dt><dd>{status.version}</dd>
              <dt>Shell</dt><dd>{status.detected_shell ?? 'Unknown'}</dd>
              <dt>Config directory</dt><dd>{status.config_directory}</dd>
              <dt>Detection source</dt><dd>{status.detection_source}</dd>
            </dl>}
          </section>
        )}
        <header><p className="eyebrow">Aliases</p><h2>All aliases</h2></header>
        <section className="empty-state" aria-label="Aliases list">
          <p>No aliases yet.</p>
          <button>Add alias</button>
        </section>
      </section>
    </main>
  )
}
```

Add only the CSS needed for the shell in `styles.css`; avoid implementing a full design system in Task 18.

- [ ] **Step 3: Commit the frontend shell**

```bash
git add crates/aliasmgr-gui/ui/src/main.ts crates/aliasmgr-gui/ui/src/App.tsx crates/aliasmgr-gui/ui/src/app.test.ts crates/aliasmgr-gui/ui/vite.config.ts crates/aliasmgr-gui/ui/src/styles.css
git commit -m "feat: add task18 gui sidebar and status drawer"
git push origin feature/alias-manager-mvp
```

---

### Task 4: Add GUI-specific CI checks without changing the CLI gate

**Files:**
- Modify: `.github/workflows/ci.yml`
- Modify: `plan.md`
- Modify: `docs/gui/README.md` only if the exact Task 18 evidence procedure needs a pointer

- [ ] **Step 1: Add Linux frontend and standalone Tauri compile jobs**

Add a GUI Linux job that:

1. Checks out the repository.
2. Installs Node using a pinned setup action.
3. Runs `npm ci` and `npm test` in `crates/aliasmgr-gui/ui`.
4. Runs `npm run typecheck` and `npm run build`.
5. Installs the Ubuntu 24.04 Tauri dependencies specified in the architecture/CI docs.
6. Runs the standalone GUI Rust tests/build from `crates/aliasmgr-gui/src-tauri` without adding it to the root workspace.

No `cargo tauri dev` step is allowed.

- [ ] **Step 2: Add Windows frontend and standalone Tauri compile jobs**

Add a GUI Windows job that runs the same frontend checks and standalone Tauri compile using the Windows runner. Keep it separate from the existing PowerShell 5.1/7 shell matrix.

- [ ] **Step 3: Update plan evidence only after both GUI jobs pass**

Under Task 18, mark only:

```markdown
- [x] 静态 GUI shell、C3 startup_status command 和 frontend test/typecheck 已实现（source/test evidence）。
- [x] 【CI】Linux GUI frontend/build 与 standalone Tauri compile 成功（run id）。
- [x] 【CI】Windows GUI frontend/build 与 standalone Tauri compile 成功（run id）。
- [ ] Linux/Windows 真实窗口启动与交互人工验收（docs/gui/ report required）。
```

Do not mark the original Task 18 broad checkbox complete while manual startup/interaction evidence is absent.

- [ ] **Step 4: Commit CI and plan evidence**

```bash
git add .github/workflows/ci.yml plan.md docs/gui/README.md
git commit -m "ci: verify task18 gui shell boundary"
git push origin feature/alias-manager-mvp
```

Run fast CI and the relevant integration workflow through `gh` first; use `"D:/Program Files/GitHub CLI/gh.exe"` only if `gh` is unavailable. Record the successful run IDs in `plan.md`.

---

## Plan self-review

- **Spec coverage:** C3 core-backed startup command is Task 2; A2/B3 UI structure is Task 3; frontend/Rust tests and CI/manual boundary are Tasks 2–4; explicit exclusions are in scope and each task.
- **Workspace boundary:** The GUI package is never added to root workspace members; CI invokes it explicitly.
- **No placeholders:** Every implementation step names exact files, APIs, tests, and commands. `plan.md` remains unchecked for manual acceptance until a user report exists.
- **Type consistency:** `StartupStatus` fields match the Rust command and TypeScript type; command name is `startup_status` in both layers; `App` invokes the same command.
- **Security:** No environment dump, profile write, string evaluation, or config-directory mutation is introduced by Task 18.
