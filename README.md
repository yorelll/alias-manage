# Alias Manager

跨平台命令别名管理器，目标平台为 Linux 与 Windows，支持 Bash、Zsh、PowerShell 5.1 和 PowerShell 7。

SQLite 是唯一事实来源，各 Shell 的生成脚本是可重建的派生文件。普通路径使用结构化参数数组，不执行整行字符串求值；用户 RC/Profile 只写入一个带标记的 loader 块。

## 功能

- 别名增删改查、启用/禁用、重命名、精确/模糊搜索、标签过滤与排序
- 别名描述/备注与标签；`list`/`find` 按标签过滤，GUI 提供标签分面
- 限行（执行目录限制）：白名单/黑名单，为每个别名生成目录守卫；见 [docs/restrictions.md](docs/restrictions.md)
- Bash、Zsh、PowerShell 5.1、PowerShell 7 适配器；简单别名使用原生 `alias`/`Set-Alias`，其余生成函数包装器
- 结构化 argv 透传、工作目录、环境变量、`{{args}}` 占位符
- 同步锁、备份与轮转、operation journal、崩溃恢复、生成文件 checksum 与语法检查
- 统一跨 Shell 导入/导出：同一份 JSON/TOML 文件（带 `format_version` 信封）可在 Bash、Zsh、PowerShell 5.1 和 PowerShell 7 之间迁移；敏感环境变量过滤，预览与确认分离
- 保留/清除式卸载；从不删除用户引用的目标文件
- Tauri 2 GUI（预发布）

## 构建要求

- Rust stable（版本见 `rust-toolchain.toml`）
- Windows：需要 MinGW-w64 `gcc` 在 `PATH` 上（`rusqlite` 使用 bundled SQLite）。Git Bash 示例：
  ```bash
  export PATH="/d/mingw64/bin:$PATH"
  ```
- Node.js 18+：仅构建 GUI 前端时需要
- Linux GUI 构建还需要 `webkit2gtk-4.1`、`libgtk-3-dev` 等 Tauri 原生依赖；仅构建 CLI 不需要
- SQLite 由 `rusqlite` 的 `bundled` feature 提供，不依赖系统 SQLite

## 构建 CLI

```bash
cargo build --release -p aliasmgr-cli
```

产物：Linux 为 `target/release/aliasmgr`，Windows 为 `target/release/aliasmgr.exe`。

## 快速开始

```bash
aliasmgr add gs --exec git --arg status --shell bash --tag git
aliasmgr list
aliasmgr sync
aliasmgr reload --print --shell bash
aliasmgr doctor
```

配置根优先级：

1. `--config-dir <PATH>`
2. `ALIASMGR_CONFIG_DIR`（空值视为未设置）
3. 平台默认：
   - Windows：`%LOCALAPPDATA%\alias-manager`，其次 `%APPDATA%\alias-manager`
   - Linux：`$XDG_CONFIG_HOME/alias-manager`，其次 `$HOME/.config/alias-manager`

已打开的父 Shell 不会被 CLI 子进程自动更新；执行 `reload --print` 输出的命令（或重新打开 Shell）后生效。

## 构建 GUI（预发布）

```bash
cd crates/aliasmgr-gui/ui
npm ci
npm run build
cd ../..
cargo build --release --manifest-path crates/aliasmgr-gui/src-tauri/Cargo.toml
```

Windows 需要 WebView2 Runtime；Linux 需要 WebKitGTK 4.1 等原生依赖。GUI 的真实窗口、视觉与交互验收仍属预发布阶段，不在当前 CLI 发布门禁内。

## 命令

`add`、`remove`、`update`、`rename`、`get`、`find`、`list`、`enable`、`disable`、`sync`、`reload`、`doctor`、`shell detect`、`shell install`、`shell uninstall`、`import`、`export`、`uninstall`。

完整参数与退出码见 [docs/cli-commands.md](docs/cli-commands.md) 和 [docs/exit-codes.md](docs/exit-codes.md)。

## 安全边界

- 普通生成路径不使用 `eval`、`Invoke-Expression` 或整行字符串求值；参数以数组形式传递
- 高级 Shell 模式与 `RawShellCommand` 被拒绝
- loader 只操作 `# >>> Alias Manager >>>` / `# <<< Alias Manager <<<` 之间的内容；标记不成对时报错
- 导入/导出过滤 `PASSWORD`、`TOKEN`、`SECRET`、`API_KEY` 等敏感环境变量
- 卸载不会删除被引用的 EXE/BAT/CMD/PY/PS1/SH/JAR 目标文件

详见 [docs/security.md](docs/security.md)。

## 文档

- [docs/cli-commands.md](docs/cli-commands.md)：CLI 命令契约
- [docs/exit-codes.md](docs/exit-codes.md)：退出码
- [docs/limitations.md](docs/limitations.md)：已知限制
- [docs/security.md](docs/security.md)：安全边界
- [docs/restrictions.md](docs/restrictions.md)：限行（执行目录限制）语义与限制
- [docs/uninstall.md](docs/uninstall.md)：卸载语义
- [docs/architecture.md](docs/architecture.md)：架构说明

## 许可

MIT，见 [LICENSE](LICENSE)。
