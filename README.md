# Alias Manager

跨平台命令别名管理器，目标平台为 Linux 与 Windows，共享 Rust 核心库，支持 Bash、Zsh、PowerShell 5.1 和 PowerShell 7。

## 项目状态

当前 pre-GUI CLI MVP 已完成可由 CI 验证的核心范围：Rust workspace、领域模型与校验、SQLite 迁移/CRUD、结构化执行器、Bash/Zsh/PowerShell 适配器、同步与 prepared-journal 恢复、CLI 生命周期、搜索/tag facet、导入导出安全报告和卸载保护。SQLite 是配置事实来源，Shell 生成文件是可重建的派生文件；普通模式使用结构化参数，不执行整行字符串求值。

验证基线：Fast CI `31875233532`，Integration `31875307387`。

这不是完整 GUI MVP 或安装器发布声明。GUI 视觉/交互、真实用户 Profile/ExecutionPolicy、oh-my-zsh 顺序、Windows symlink、package hook/MSI 和干净机器安装验收仍需单独人工或后续 CI 阶段。完整边界见 `docs/release-checklist.md`，Task 1–17 证据对账见 `docs/phase4-reconciliation.md`。先前的设计草稿仍保留在 `plan.md`，阶段执行清单在 `docs/superpowers/plans/`。

### 当前 CLI 命令

`add`、`remove`、`update`、`rename`、`get`、`find`、`list`、`enable`、`disable`、`sync`、`reload`、`doctor`、`shell detect`、`shell install`、`shell uninstall`、`import`、`export`、`uninstall`。

所有 CI 验证使用隔离的临时配置目录；已打开的父 Shell 不会由子进程自动更新，执行生成文件前请使用 `reload --print` 输出的命令。

### 重要文档

- `docs/testing.md`：CI-only Rust/Shell 验证边界
- `docs/ci-workflow.md`：workflow、隔离和 gh 迭代规则
- `docs/security.md`：结构化参数和导入/卸载安全边界
- `docs/limitations.md`：PowerShell、Batch/CMD、交互式 Shell 限制
- `docs/release-checklist.md`：pre-GUI 发布清单与人工验收边界
- `docs/phase4-reconciliation.md`：Tasks 1–17 证据状态
- `docs/uninstall.md`：retain/purge 和 package hook/MSI 边界

> Windows/Linux GUI 和安装包仍不属于当前 pre-GUI CLI gate。

## 开发验证

本项目采用 GitHub Actions 做 Rust、Shell 和跨平台验证。由于本地环境可能没有 Rust 工具链，请不要在本地运行 Cargo 命令；提交到 feature 分支后由 CI 验证。前端自检可使用 npm，夹具语法可使用 Python。

## 重要限制

- Bash、Zsh、Fish 和 PowerShell 是 Shell/命令解释器，不是终端类型。
- 别名只保证交互式 Shell 生效，已打开的父 Shell 需要用户显式 reload。
- MVP 拒绝高级 Shell 模式与 RawShellCommand。
- 不支持别名递归解析，不自动修改父 Shell 状态。

详见 `docs/architecture.md`、`plan.md` 和后续安全、测试、限制文档。
