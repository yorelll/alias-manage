# Testing

Rust workspace、Shell 适配器和跨平台测试由 GitHub Actions 执行。本地环境没有 Rust 工具链，因此不运行 Cargo 命令。CI 测试必须使用临时 HOME、配置目录和 Profile，不能修改 runner 真实用户配置。

## Current evidence baseline

- Fast CI `31875233532`: Linux tests, Windows tests, and lint passed.
- Integration `31875307387`: Bash/Zsh, PowerShell 5.1/7, lock, CLI isolation, and matrix jobs passed.
- Later focused runs are recorded in `plan.md` beside the corresponding Task 8–17 evidence items.

The integration matrix intentionally uses separate Linux Bash/Zsh and Windows PowerShell 5.1/7 jobs. It must not print complete profiles, environment dumps, tokens, or sensitive values.

## What CI does not prove

CI does not replace GUI visual/interaction acceptance, true terminal behavior, real user Profile/ExecutionPolicy compatibility, OneDrive redirection, oh-my-zsh ordering, Windows symlink behavior, package hooks, MSI behavior, or clean-machine installation. Those remain manual acceptance items in `docs/release-checklist.md`.

GUI 的视觉和真实终端行为仍需下载构建产物后人工确认。
