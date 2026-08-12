# Alias Manager

跨平台命令别名管理器，目标平台为 Linux 与 Windows，共享 Rust 核心库，支持 Bash、Zsh、PowerShell 5.1 和 PowerShell 7。

## 项目状态

当前已完成 M0/M1 基础闭环：Rust workspace、别名领域模型、名称与参数校验、SQLite 迁移/CRUD、CLI 命令解析和 GitHub Actions 验证骨架。后续阶段再加入 Shell 生成、同步回滚和 Tauri GUI。SQLite 是配置事实来源，Shell 生成文件是可重建的派生文件；普通模式使用结构化参数，不执行整行字符串求值。先前的设计草稿仍保留在 `plan.md`，阶段执行清单在 `docs/superpowers/plans/`。

## 开发验证

本项目采用 GitHub Actions 做 Rust、Shell 和跨平台验证。由于本地环境可能没有 Rust 工具链，请不要在本地运行 Cargo 命令；提交到 feature 分支后由 CI 验证。前端自检可使用 npm，夹具语法可使用 Python。

## 重要限制

- Bash、Zsh、Fish 和 PowerShell 是 Shell/命令解释器，不是终端类型。
- 别名只保证交互式 Shell 生效，已打开的父 Shell 需要用户显式 reload。
- MVP 拒绝高级 Shell 模式与 RawShellCommand。
- 不支持别名递归解析，不自动修改父 Shell 状态。

详见 `docs/architecture.md`、`plan.md` 和后续安全、测试、限制文档。
