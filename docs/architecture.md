# Architecture

Alias Manager is a Rust workspace. `aliasmgr-core` owns the domain model, validation, storage, execution and shell adapters; CLI and GUI layers call the core API.

The MVP targets Bash, Zsh, PowerShell 5.1 and PowerShell 7. Tauri 2 Linux builds require WebKitGTK 4.1 and related GTK dependencies; CI uses Ubuntu 24.04 for compile checks. Linux packaging targets Ubuntu 22.04 for glibc compatibility.

SQLite with the bundled feature is the configuration source of truth. Shell files are rebuildable derived artifacts.
