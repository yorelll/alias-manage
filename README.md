# Alias Manager

跨平台命令别名管理器，目标平台为 Linux 与 Windows，共享 Rust 核心库，支持 Bash、Zsh、PowerShell 5.1 和 PowerShell 7。

## 项目状态

当前 pre-GUI CLI MVP 已完成可由 CI 验证的核心范围：Rust workspace、领域模型与校验、SQLite 迁移/CRUD、结构化执行器、Bash/Zsh/PowerShell 适配器、同步与 prepared-journal 恢复、CLI 生命周期、搜索/tag facet、导入导出安全报告和卸载保护。SQLite 是配置事实来源，Shell 生成文件是可重建的派生文件；普通模式使用结构化参数，不执行整行字符串求值。

本地最新基线（2026-11-01）：workspace 172 个测试通过，GUI Tauri crate 14 个测试通过，GUI 前端 215 个测试及 typecheck/build 通过。当前分支尚未推送触发 CI；跨平台 Shell、打包与发布结论仍以 GitHub Actions 为准。最新状态见 `docs/final-release-status.md`。

这不是完整 GUI MVP 或安装器发布声明。GUI 视觉/交互、真实用户 Profile/ExecutionPolicy、oh-my-zsh 顺序、Windows symlink、package hook/MSI 和干净机器安装验收仍需单独人工或后续 CI 阶段。完整边界见 `docs/release-checklist.md`，Task 1–17 证据对账见 `docs/phase4-reconciliation.md`。先前的设计草稿仍保留在 `plan.md`，阶段执行清单在 `docs/superpowers/plans/`。

### 当前 CLI 命令

`add`、`remove`、`update`、`rename`、`get`、`find`、`list`、`enable`、`disable`、`sync`、`reload`、`doctor`、`shell detect`、`shell install`、`shell uninstall`、`import`、`export`、`uninstall`。

所有 CI 验证使用隔离的临时配置目录；已打开的父 Shell 不会由子进程自动更新，执行生成文件前请使用 `reload --print` 输出的命令。

### 重要文档

- `docs/testing.md`：本地 Rust/前端预检命令与 CI 跨平台验证边界
- `docs/ci-workflow.md`：workflow、隔离和 gh 迭代规则
- `docs/security.md`：结构化参数和导入/卸载安全边界
- `docs/limitations.md`：PowerShell、Batch/CMD、交互式 Shell 限制
- `docs/release-checklist.md`：pre-GUI 发布清单与人工验收边界
- `docs/phase4-reconciliation.md`：Tasks 1–17 证据状态
- `docs/uninstall.md`：retain/purge 和 package hook/MSI 边界

> Windows/Linux GUI 和安装包仍不属于当前 pre-GUI CLI gate。

## 开发验证

本机已有完整 Rust 工具链（cargo/rustc + MinGW gcc）和 Node/npm，编码阶段可在本地做快速预检。Git Bash 下先设置：

```bash
export PATH="/d/mingw64/bin:/d/Program Files/nodejs:$PATH"
export ALIASMGR_CONFIG_DIR="$PWD/target/tcfg"
mkdir -p "$ALIASMGR_CONFIG_DIR"
```

然后运行 `cargo check --workspace --all-targets`、`cargo test --workspace --no-fail-fast`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo fmt --all -- --check`；GUI 前端在 `crates/aliasmgr-gui/ui` 下运行 `npm ci && npm run typecheck && npm test && npm run build`。

本地通过只是预检，不是发布证据。跨平台 Shell 行为、集成测试、打包和 release 结论仍以 GitHub Actions 为准；未推送的分支不得声称 CI 通过。完整命令与边界见 `docs/testing.md`。

## 重要限制

- Bash、Zsh、Fish 和 PowerShell 是 Shell/命令解释器，不是终端类型。
- 别名只保证交互式 Shell 生效，已打开的父 Shell 需要用户显式 reload。
- MVP 拒绝高级 Shell 模式与 RawShellCommand。
- 不支持别名递归解析，不自动修改父 Shell 状态。

详见 `docs/architecture.md`、`plan.md` 和后续安全、测试、限制文档。
