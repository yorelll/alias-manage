# Alias Manager

跨平台命令别名管理器，目标平台为 Linux 与 Windows，支持 Bash、Zsh、PowerShell 5.1 和 PowerShell 7。

SQLite 是唯一事实来源，各 Shell 的生成脚本是可重建的派生文件。CLI、TUI 与 GUI 通过同一配置根读写同一个 `<config-root>/aliases.db`，不维护多份数据库。普通路径使用结构化参数数组，不执行整行字符串求值；用户 RC/Profile 只写入一个带标记的 loader 块。

## 功能

- 别名增删改查、启用/禁用、重命名、精确/模糊搜索、标签过滤与排序
- 别名描述/备注与标签；`list`/`find` 按标签过滤，GUI 提供标签分面
- 限行（执行目录限制）：白名单/黑名单，为每个别名生成目录守卫；见 [docs/restrictions.md](docs/restrictions.md)
- Bash、Zsh、PowerShell 5.1、PowerShell 7 适配器；简单别名使用原生 `alias`/`Set-Alias`，其余生成函数包装器
- 结构化 argv 透传、工作目录、环境变量、`{{args}}` 占位符
- 同步锁、备份与轮转、operation journal、崩溃恢复、生成文件 checksum 与语法检查
- 统一跨 Shell 导入/导出：同一份 JSON/TOML 文件（带 `format_version` 信封）可在 Bash、Zsh、PowerShell 5.1 和 PowerShell 7 之间迁移；敏感环境变量过滤，预览与确认分离
- 保留/清除式卸载；从不删除用户引用的目标文件
- 统一终端 UI（`aliasmgr tui`）：列表/搜索/标签过滤/排序、创建/编辑/重命名/启用/禁用/删除、结构化参数编辑、生成预览、后台同步、Doctor、导入/导出与共享 `config.toml` 编辑；Bash/Zsh/PowerShell 5.1/PowerShell 7 共用一个 TUI
- 保存即完成：`add`/`update`/`remove`/`rename`/`enable`/`disable` 成功后自动同步受影响的 Shell；`--no-sync` 关闭单次自动同步，`runtime.auto_sync = false` 全局关闭
- 首次 Shell 集成：交互式 `aliasmgr doctor` 检测四个 Shell、勾选启用、预览并安装 loader、执行同步并显示 reload 命令；非 TTY 保留文本/JSON 输出
- Tauri 2 GUI（预发布）

## 构建要求

- Rust stable（版本见 `rust-toolchain.toml`）
- Windows：需要 MinGW-w64 `gcc` 在 `PATH` 上（`rusqlite` 使用 bundled SQLite）。Git Bash 示例：
  ```bash
  export PATH="/d/mingw64/bin:$PATH"
  ```
- Node.js 18+：仅构建 GUI 前端时需要
- Linux GUI 构建还需要 `webkit2gtk-4.1`、`libgtk-3-dev` 等 Tauri 原生依赖；仅构建 CLI 不需要
- SQLite 由 `rusqlite` 的 `bundled` feature 提供（当前 3.54.1），不依赖系统 SQLite；SQLite 3.7.0–3.51.2（不含 3.44.6/3.50.7 backport）存在 WAL reset 多连接损坏缺陷；本项目要求/内置 >=3.51.3（当前 3.54.1）

## 构建 CLI

```bash
cargo build --release -p aliasmgr-cli
```

产物：Linux 为 `target/release/aliasmgr`，Windows 为 `target/release/aliasmgr.exe`。

默认构建包含 `tui` feature（ratatui + crossterm）。只构建非交互 CLI：

```bash
cargo build --release -p aliasmgr-cli --no-default-features
```

该模式下不链接任何 TUI 依赖，`aliasmgr tui` 会明确提示未编译 TUI 支持并以退出码 1 结束。交互式 `aliasmgr doctor` 会回退为逐行提示（仍可完成检测、路径填写、预览、安装与同步），不会链接 crossterm。现有非交互命令、参数、输出与退出码不因 TUI 改变。

## 快速开始

```bash
aliasmgr doctor
aliasmgr add gs --exec git --arg status --shell bash --tag git
aliasmgr list
aliasmgr sync
aliasmgr reload --print --shell bash
aliasmgr tui
```

首次使用先运行 `aliasmgr doctor`：它列出全部四个受支持 Shell（即使未检测到），由用户确认后启用并安装 loader。没有单独的 `setup` 命令。未启用的 Shell 或 loader 无效时，`add`/`update` 会在写库前拒绝并提示运行 `doctor`；`--allow-missing-loader` 可显式保存并警告（该 Shell 暂不生效）。

配置根优先级：

1. `--config-dir <PATH>`
2. `ALIASMGR_CONFIG_DIR`（空值视为未设置）
3. 平台默认：
   - Windows：`%LOCALAPPDATA%\alias-manager`，其次 `%APPDATA%\alias-manager`
   - Linux：`$XDG_CONFIG_HOME/alias-manager`，其次 `$HOME/.config/alias-manager`

已打开的父 Shell 不会被 CLI 子进程自动更新；执行 `reload --print` 输出的命令（或重新打开 Shell）后生效。

### Shell 集成与保存即完成

`config.toml` 保存用户意图，loader 是否有效每次运行时重新检测：

```toml
[runtime]
auto_sync = true

[shells]
enabled = ["powershell5", "powershell7"]   # 稳定拼写：bash|zsh|powershell5|powershell7
# 现有 profile 路径覆盖继续有效
powershell7_profile_path = "C:\\Users\\me\\Documents\\PowerShell\\Microsoft.PowerShell_profile.ps1"

[shells.executables]                       # 可选：非标准安装路径
powershell7 = "C:\\Program Files\\PowerShell\\7\\pwsh.exe"
bash = "C:\\Program Files\\Git\\bin\\bash.exe"
```

- `enabled` 默认空；加载旧配置不会自动启用任何 Shell，只有用户在 `doctor`/设置中确认后才写入。
- `runtime.auto_sync` 默认 `true`。`add`/`update`/`remove`/`rename`/`enable`/`disable` 成功后只同步一次受影响的 Shell；任一 Shell 同步失败时数据库已保存，命令以 `PartialSync`（退出码 10）返回并列出失败 Shell。`--no-sync` 跳过本次自动同步；`runtime.auto_sync = false` 全局关闭。
- 自动同步不依赖 `enabled`：生成文件内容与 `enabled` 无关；loader 只对已启用 Shell 安装。
- 检测来源：`PATH`、Windows 已知安装路径（PS5 系统路径、PS7 Program Files/WindowsApps、Git Bash、MSYS2/Cygwin Zsh）、Unix `/bin`、`/usr/bin`、`/usr/local/bin`，以及 `shells.executables` 手动路径（最高优先级）。版本探测有约 2 秒超时；失败只表示版本未知，不表示未安装。
- 无法保证发现所有安装方式（便携版、Store 别名、启动后安装、WSL、不同 PATH）。`doctor` 始终列出四个 Shell 并允许手动填写路径。
- Windows `System32\bash.exe`/WindowsApps `bash.exe` 会标记为 WSL；不会向 Windows profile 写入 WSL 的 loader，请在 WSL 发行版内运行 `doctor`。

- Bash/Zsh 使用 `source '<config-root>/generated/bash.sh'`。
- PowerShell 使用 `. '<config-root>\generated\powershell7.ps1'`；开头的 `.` 是 **点源（dot-source）运算符**，不是逗号，表示在当前会话中运行该文件。

重载会让当前 Shell 重新读取生成文件，适用于别名变更后或在已打开的 Shell 中同步后；新 Shell/终端会通过 loader 自动加载。重载不会修改数据库。

目标类型（`--target-type`）只是分类/校验元数据，不会自动添加解释器。要运行 `python test.py`，请设置 `--exec python --arg test.py`，不要写成 `--exec "python test.py"`。各类型的运行时行为见 [docs/cli-commands.md](docs/cli-commands.md#target-type-semantics)。

## 终端 UI（TUI）

```bash
aliasmgr tui
aliasmgr --config-dir /tmp/aliasmgr-demo tui
```

需要交互式终端（stdin/stdout 均为 TTY）；管道、CI 或重定向输出中会立即报错退出（退出码 1），不会挂起或输出转义序列。最小可用尺寸 80x24；≥120 列时列表与详情双栏，80–119 列窄屏用 `Enter` 打开详情、`Tab` 在列表/详情间切换焦点、`Esc` 返回；更小尺寸只接受 `q`/`Ctrl+C` 并显示明确的“终端太小”提示，其余按键不会触发任何操作。

常用键：`↑/↓` 或 `j/k` 移动，`Enter` 详情，`/` 搜索，`n` 新建，`e` 编辑，`r` 重命名，`Space` 启用/禁用，`d` 删除，`s` 后台同步，`p` 按别名预览（`f` 切换整文件回退），`D` Doctor，`t` 标签过滤，`o`/`O` 排序，`R` 手动刷新，`I` 导入，`X` 导出，`S` 编辑共享配置，`?` 帮助，`q`/`Ctrl+C` 退出；同步结果页 `r` 仅重试失败的 Shell，`Esc` 清除已应用的搜索/标签/排序筛选。编辑时 `Tab` 切换字段、`Ctrl+S` 保存、`Esc` 取消；固定参数逐行编辑（空格、引号、空字符串、CJK 与 `{{args}}` 原样保留，不做拆分或求值）。

TUI 复用同一 SQLite、校验、同步、Doctor、loader 与 Shell 适配器；保存只写数据库并提示需要同步，同步后仍需用户手动执行显示的 reload 命令。详情区区分四层状态：DB 已写入 / 生成文件已更新 / loader 已安装 / 当前终端是否已加载（未知）。环境变量值默认显示为 `***`，生成预览在渲染前即脱敏。

完整快捷键、目标类型语义与限制见 [docs/tui.md](docs/tui.md)。
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

`add`、`remove`、`update`、`rename`、`get`、`find`、`list`、`enable`、`disable`、`sync`、`reload`、`doctor`、`tui`、`shell detect`、`shell install`、`shell uninstall`、`import`、`export`、`uninstall`。

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
- [docs/tui.md](docs/tui.md)：TUI 使用说明、快捷键、目标类型语义与限制

## 许可

MIT，见 [LICENSE](LICENSE)。
