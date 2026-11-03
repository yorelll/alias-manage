# R2.0 Handoff：保存即完成 + 首次 Shell 集成

> 目标读者：实现 Agent A3、Review Agent B3、功能测试 Agent C3。
> 状态：已确认排期（R2.1 通过后、R2.1b 之前）。
> 原则：用户只关心“在哪些 Shell 里使用别名”；sync/loader/reload 的内部层次不应暴露为日常步骤。不新增 `setup` 命令；GUI/TUI 首次引导，CLI 使用交互式 `doctor`。

## 0. 已确认的产品行为

1. **保存即完成（auto-sync）**：`add/update/remove/enable/disable/rename` 成功后自动同步受影响的 Shell；`--no-sync` 逃生；配置 `runtime.auto_sync = true` 默认开启。
2. **首次 Shell 集成**：GUI/TUI 首次打开让用户勾选要启用的 Shell；对 detected 的 Shell 自动安装 loader（用户只需确认）；对 not detected 的 Shell 提供“安装并加 PATH 后重启 / 手动填路径 / 取消”三选项。
3. **后续启动**：只检查已 enabled Shell 的 loader；失效时提示“X 集成已失效，请在设置中重新启用”，提供 重新启用 / 暂时忽略 / 禁用；不每次弹全屏向导。
4. **后装 Shell**：下次启动检测到新 Shell 时提示一次启用；也可在设置页或 `doctor` 中启用；无需重跑 setup。
5. **添加别名预检**：选择未 enabled 的 Shell 时保存拦截，进入同一启用流程；拒绝启用则保留草稿，不保存无效别名。
6. **CLI**：`doctor` 在 TTY 下进入交互式 Shell 集成界面；非 TTY 保持现有文本输出。`add/update` 保存前检查 enabled + loader valid，默认严格，提供 `--allow-missing-loader` 逃生。
7. **当前终端 reload**：仍无法由子进程代做；只展示/复制 reload 命令。新终端通过 loader 自动生效。

## 1. 现状事实（先读代码确认，以下为 2026-11-03 事实）

### 检测
- `crates/aliasmgr-core/src/detection.rs`
  - `installed_shells()` **只查 PATH**：`bash`、`zsh`、`powershell`、`pwsh`。
  - `installed_powershells()` 查 PATH 并运行 `$PSVersionTable` 获取版本。
  - `detect(context)` 支持 explicit/parent/shell_env/login_shell，并回退 `installed.first()`。
  - 没有已知安装路径、没有 WSL 区分、没有手动 executable 路径。
- 结论：PS7 已安装但 `pwsh` 不在 PATH、GUI/TUI 启动后才安装、WSL bash、便携版都会漏检。

### 配置
- `crates/aliasmgr-core/src/config.rs`
  - `AppConfig { backups, logs, retired_names, shells: ShellPathOverrides, runtime: ShellRuntimeConfig }`。
  - `ShellPathOverrides { bash_rc_path, zsh_rc_path, powershell5_profile_path, powershell7_profile_path }`。
  - `ShellRuntimeConfig { default_shell, language, allow_relative_paths, execution_policy, powershell*_bom, line_ending }`。
  - `AppConfig::validate` 已校验 default_shell/language/keep 上限；`AppPaths::load_config/save`。
  - `save` 目前使用 `fsutil::write_atomic`（R2.1 已加入）。
- `ShellKind` serde 是 `power_shell5`/`power_shell7`，但 `ShellKind::as_str()` 是 `powershell5`/`powershell7`。配置里存 enabled 列表时**必须用 `as_str()` 的稳定值**，不要用 serde 拼写；读取时用显式映射。

### loader / 集成
- `crates/aliasmgr-core/src/integration.rs`
  - `loader_target(config, paths, shell)` → (profile/RC 路径, powershell bool)。
  - `loader_candidates` / `all_loader_targets`（configured + fallback，dedupe）。
  - `loader_status`：比较精确标记块 + 位置；区分 `loader_permission`/`loader_unreadable`/`loader_io`/`loader_unsafe`/`loader_shared_path` 等。
  - `install_loader` / `uninstall_loader`：只改标记块、备份/保留、原子写、畸形 marker 拒绝。
- `crates/aliasmgr-core/src/doctor.rs`：`diagnose()` 已探测真实 loader；CLI/GUI 共用。

### 同步
- `crates/aliasmgr-core/src/sync.rs`
  - `referenced_shells`（aliases + tombstones）、`current_revision`、`sync_config_dir*`、`sync_shells`（子集）、`reload_command`、`shell_content_status`、`journal_needs_recovery`。
  - `SyncCoordinator` 处理锁/journal/备份/原子写/语法检查。
- CLI `commands::sync` 与 GUI `sync_aliases` 已复用 shared sync；TUI 后台作业也复用。

### 前端
- CLI：`aliasmgr` 子命令 add/update/remove/enable/disable/rename/doctor/shell install/uninstall；`atty` 可用；help 已方案 B 前的英文。
- GUI：Tauri commands `sync_aliases`、`shell_install`、`doctor_status`、`config_get/save`、`generated_preview`；UI 有 Sync/Doctor/Settings 面板。
- TUI：`crates/aliasmgr-tui`；`App` 状态机、后台 Jobs、Settings/Import/Export 屏幕；已有 `integration`/`sync`/`doctor` 复用。
- `fsutil::write_atomic`：同目录临时文件 + rename，已用于 export/config（R2.1）。

## 2. 共享状态设计

### 2.1 配置 schema（建议）

```toml
[runtime]
auto_sync = true

[shells]
enabled = ["powershell5", "powershell7"]   # ShellKind::as_str() 值，稳定
# 现有 profile 覆盖保持不变
bash_rc_path = "..."
powershell7_profile_path = "..."

[shells.executables]                       # 可选；非标准安装路径
powershell7 = "C:\\Program Files\\PowerShell\\7\\pwsh.exe"
bash = "C:\\Program Files\\Git\\bin\\bash.exe"
```

- `enabled` 是**用户意图**；loader 是否有效是每次检测的事实，不持久化。
- 默认 `enabled = []`，首次引导让用户选择；不要静默 enable 所有 detected Shell。
- 迁移：若旧配置无 `enabled`，不要自动写；首次启动根据已有 loader 展示建议（已安装 loader 的 Shell 预勾选）。
- `runtime.auto_sync` 默认 true；`serde(default)` 兼容旧配置。
- 未知字段保留（R2.1 已实现 top-level 未知字段 round-trip；嵌套未知字段暂不保留，文档说明）。

### 2.2 状态模型（不要混淆）

| 状态 | 来源 | 用途 |
|---|---|---|
| detected | 检测（PATH + 已知路径 + 手动路径） | 提示可用性；不自动 enable |
| enabled | `shells.enabled` 用户意图 | 决定是否生成/安装/提示 |
| loader valid | `integration::loader_status` 实时 | 决定新终端是否自动加载 |
| auto_sync | `runtime.auto_sync` | 保存后是否自动同步 |
| current session loaded | 无法可靠确认 | 永远显示“需要手动 reload/未确认” |

### 2.3 建议新增共享 API（避免 GUI/TUI/CLI 三份逻辑）

在 core 增加（名称可调整）：

```rust
pub struct ShellInstallation {
    pub kind: ShellKind,
    pub executable: Option<PathBuf>,
    pub detected: bool,
    pub source: ShellDetectionSource, // path/known-path/manual/wsl
    pub version: Option<String>,
    pub wsl: bool,
}

pub fn detect_shells(config: &AppConfig) -> Vec<ShellInstallation>;
pub fn enabled_shells(config: &AppConfig) -> Vec<ShellKind>;
pub fn set_enabled_shells(paths, &[ShellKind]) -> Result<()>;

pub struct ShellSetupPlan {
    pub kind: ShellKind,
    pub detected: bool,
    pub executable: Option<PathBuf>,
    pub loader_target: Option<PathBuf>,
    pub loader_status: LoaderStatus,
    pub action: SetupAction, // Enable/InstallLoader/NeedsPath/Disable
}
pub fn plan_shell_setup(config, paths, kind, executable_override, profile_override) -> Result<ShellSetupPlan>;
pub fn apply_shell_setup(...) -> Result<SetupOutcome>; // enable + install loader + sync
```

- 所有 frontend 只调用这些函数；不要复制路径解析/标记块/同步。
- `apply_shell_setup` 必须在用户确认后调用；安装前返回可预览的 block/paths。

## 3. 检测设计

### 3.1 检测来源

- PATH 探测：`bash`、`zsh`、`powershell`、`pwsh`。
- Windows 已知路径：
  - PS5：`%SystemRoot%\System32\WindowsPowerShell\v1.0\powershell.exe`
  - PS7：`%ProgramFiles%\PowerShell\7\pwsh.exe`、`%LOCALAPPDATA%\Microsoft\WindowsApps\pwsh.exe`；也考虑 `PowerShell\7-preview`。
  - Bash：`C:\Program Files\Git\bin\bash.exe`、`C:\Program Files\Git\usr\bin\bash.exe`、MSYS2 `C:\msys64\usr\bin\bash.exe`。
  - Zsh：`C:\msys64\usr\bin\zsh.exe`、Cygwin 常见路径。
  - WSL：`%SystemRoot%\System32\bash.exe` / WindowsApps `bash.exe`；标记 `wsl=true`，不要自动写 Windows profile。
- Unix：PATH + `/bin`/`/usr/bin`/`/usr/local/bin` + `$SHELL`（仅提示，不自动 enable）。
- 手动 `shells.executables.<shell>` 覆盖优先级最高；用户可在首次引导/设置/doctor 填写。
- 版本探测：PowerShell `-NoProfile -NonInteractive -Command $PSVersionTable.PSVersion.ToString()`；bash/zsh `--version`；设置超时（如 2s），失败只标记 unknown，不影响路径存在。只读，不改环境。

### 3.2 检测边界（必须写入文档/UI）

- 无法保证 100% 自动发现：PATH 差异、Store/非标准安装、进程启动后安装、WSL、便携版、不同启动方式。
- 因此**始终列出全部 4 个 Shell**；detected=false 也要显示，并允许手动路径。
- `detected` 不等于 `enabled`；`enabled` 不等于 `loader valid`。
- WSL bash 的 profile 在发行版内，Windows 侧不能安全写入；提示“请在 WSL 内安装/运行 doctor”。
- 若别名 targets 的 Shell 未 enabled/loader 无效，明确警告；不要显示“已生效”。

## 4. GUI/TUI 首次引导

### 4.1 首次打开

- 列出四行：`PowerShell 5.1`、`PowerShell 7`、`Bash`、`Zsh`，每行显示 detected/path 或 not detected。
- 默认勾选：已有有效 loader 的 Shell；否则不勾选（或按 detected 预选，但需用户确认）。
- 操作：`Space` 勾选、`Enter` 应用、`r` rescan、`q` 退出/稍后。
- detected + 勾选：预览 profile/RC 路径 + 标记块 → 确认 → 安装 loader + 自动 sync；显示 reload 命令。
- not detected + 勾选：三选项（同 CLI）：
  1. 安装该 Shell 并加入 PATH，重启软件；
  2. 已安装但路径非标准：填写 executable + profile/RC（默认值/使用默认路径），无需重启；
  3. 取消该 Shell enable。
- 未勾选：`enabled=false`；后续不主动骚扰。
- 首次流程必须可跳过/稍后；不阻塞进入主界面。

### 4.2 后续启动

- 只检查 enabled Shell 的 loader：
  - valid → 静默。
  - invalid/missing → 顶部提示或一次性弹窗：“PowerShell 7 集成已失效，请在设置中重新启用。” 按钮：重新启用 / 暂时忽略 / 禁用。
  - “暂时忽略”只本次会话；“禁用”写 enabled=false。
- 新 detected 但未 enabled 的 Shell：提示一次“检测到 Bash，是否启用？”，不强制。
- 在设置页提供 Shell 集成面板：enable/disable、检测、路径、安装/卸载 loader、rescan。

### 4.3 添加/编辑别名

- Shell 多选显示 `[enabled]` / `[disabled]` / `[not installed]`。
- 选择未 enabled 的 Shell：保存时拦截，提示“X 未启用，请先在设置中启用”，进入同一 enable 流程。
- 拒绝启用：保留草稿，不保存该 Shell 或整体阻止（推荐整体阻止，避免用户以为已生效）。
- 目标 Shell 已 enabled 但 loader 无效：同样拦截或警告，指向修复流程。
- 提供“仅保存到数据库，稍后启用”的明确选项（高级；默认不提供，避免误解）。

## 5. CLI doctor 交互式

### 5.1 TTY 判定

- `stdin` 和 `stdout` 都是 TTY：交互式。
- 否则：保持现有文本输出；可增加 `--format json`；不进入 raw mode、不等待输入。
- 不破坏脚本/CI；`doctor` 的退出码保持现有语义。

### 5.2 交互界面

```text
Detecting shells...
[ ] PowerShell 5.1   path: C:\Windows\System32\...\powershell.exe
[ ] PowerShell 7     path: C:\Program Files\PowerShell\7\pwsh.exe
[ ] Bash             path: C:\Program Files\Git\bin\bash.exe
[ ] Zsh              not found

↑/↓ select   Space enable   Enter apply   r rescan   q quit
```

- 真实检测状态/spinner；不伪造百分比；探测 PowerShell 版本可能慢，逐项显示。
- detected + Enter：预览 + 安装 loader + sync + reload 命令。
- not detected + Enter：三选项（安装并重启 / 填写路径 / 取消）。
- 填写路径：两个字段（executable、profile/RC），默认值/使用默认路径；安装前预览。
- 安装后自动 sync；显示每 Shell 结果。
- 可重复执行；只写缺失/不匹配的 loader，幂等。
- 建议复用 TUI/crossterm（CLI 已有可选 `aliasmgr-tui` 依赖），避免新增依赖；未启用 TUI feature 时回退到简单行式提示（仍可完成）。

### 5.3 CLI add/update 预检

- 写库前检查目标 Shell：
  - 未 enabled → 拒绝保存，提示 `aliasmgr doctor`；退出码用 `ShellNotInstalled`（12）或新增明确错误（保持文档映射）。
  - enabled 但 loader invalid → 拒绝或警告（推荐拒绝），提供 `--allow-missing-loader` 显式跳过。
- `--allow-missing-loader` 时仍保存并提示“该别名在目标 Shell 中暂不生效；请运行 doctor”。
- `--no-sync` 时只保存，不检查 loader？建议仍检查 enabled，但跳过 loader 检查；文档说明。
- 不检查非目标 Shell。

## 6. auto-sync 语义

- 触发：add/update/remove/enable/disable/rename 成功后；GUI/TUI 保存/删除/启停/重命名后。
- 受影响 Shell：
  - add/update：old ∪ new shells；
  - remove/rename：old shells（tombstone）；
  - enable/disable：包含该 Shell（若生成内容受 enabled 影响；先确认 renderer 行为）。
- 只同步 `enabled` 的 Shell？建议：同步目标 Shell，即使未 enabled 也生成文件；但首次集成只对 enabled 安装 loader。
- 失败：
  - 明确显示“已保存到数据库；以下 Shell 同步失败：…”。
  - 默认返回 `PartialSync`（exit 10）以便脚本感知；或提供 `--sync=strict|auto|off`（默认 strict）。不要吞掉失败。
  - `--no-sync` 返回保存结果，exit 0。
  - 锁被占用：等待现有超时；超时则报“同步锁被占用，请稍后运行 sync/doctor”。
- 一次命令只 sync 一次；批量/导入后 sync 一次。
- 不自动 reload；不自动重启 Shell。
- GUI/TUI 后台作业复用现有 Jobs/线程；不阻塞 UI。

## 7. 风险与缓解

| 风险 | 缓解 |
|---|---|
| 检测不到全部 Shell | 始终列 4 个 + 已知路径 + 手动 executable/profile + rescan + 目标校验 |
| 启动弹窗骚扰 | enabled/disabled 持久化；失效只提示一次；可忽略/禁用 |
| 用户填错 profile | 预览、备份/保留、只改标记块、畸形 marker 拒绝 |
| CLI 脚本被交互破坏 | 严格 TTY；非 TTY 文本/JSON；add 预检可用 `--allow-missing-loader` |
| auto-sync 失败被吞 | 明确状态 + 默认 PartialSync/exit 10 + `--no-sync`/`--sync` |
| 多入口并发改配置 | 共享 config + 原子写 + 启动/刷新重读；不引入第二事实来源 |
| WSL bash 误写 Windows profile | 标记 WSL，不自动安装；提示在发行版内执行 |
| 版本探测卡住 | 2s 超时；失败只标 unknown；不阻塞 UI |
| 用户拒绝启用但仍保存 | 保存拦截，保留草稿；不静默保存无效别名 |
| Shell 卸载后 loader 残留 | 启动检测提示“已启用但未检测到”；提供禁用/清理 |

## 8. 测试矩阵

- 配置：`enabled` 持久化/迁移/未知字段保留/原子写；`auto_sync` 默认 true。
- 检测：PATH、已知路径、手动路径、WSL 标记、后装 Shell、超时、无效路径。
- 首次引导：detected/not detected、三选项、取消、跳过、预览/确认、仅写缺失 loader。
- 后续启动：loader 有效静默；失效提示；暂时忽略/禁用；新 detected 提示一次。
- 添加别名：未 enabled 拦截；enabled 但 loader invalid 拦截/`--allow-missing-loader`；拒绝启用保留草稿。
- auto-sync：成功、部分失败、锁占用、`--no-sync`/`--sync`、CLI 退出码、TUI/GUI 后台结果。
- CLI doctor：TTY 交互（PTY）、非 TTY 文本/JSON、路径填写、rescan、安装后 sync/reload。
- 安全：不写标记块外内容、不写环境变量/注册表、敏感值不泄露、真实 profile 不被测试修改。
- 真实 PTY：Linux Bash/Zsh、Windows PS5.1/pwsh7；终端恢复/退出。
- 文档：README/docs/tui.md/docs/cli-commands.md/architecture/limitations 更新；明确“无法保证自动发现全部 Shell”。

## 9. 验收标准（用户视角）

1. 全新配置首次打开 GUI/TUI：勾选 PS5/PS7 → 自动安装 loader + sync；添加 `gs` → 新开 PowerShell 直接可用；当前终端显示 reload 命令。
2. PS7 已装但未检测到：首次引导可手动填 executable/profile 并安装，无需重启。
3. 之后安装 Bash：下次启动提示启用；确认后安装 loader + sync；新 Bash 可用。
4. 用户手动删除 loader：下次启动提示“集成已失效”，可重新启用/忽略/禁用。
5. 添加别名选择未 enabled 的 PS7：保存被拦截并进入启用流程；拒绝则草稿保留。
6. CLI 非 TTY `doctor` 输出保持文本/JSON；交互式 `doctor` 可完成启用；`add` 预检与 `--allow-missing-loader` 行为正确。
7. auto-sync 部分失败：保存成功但明确列出失败 Shell；退出码可被脚本感知；`--no-sync` 不触发同步。
8. 不自动 reload/重启；不写标记块外内容；不泄露敏感值；环境/注册表不变。

## 10. 明确不做

- 不新增 `setup` 子命令。
- 不自动 enable 未确认的 Shell。
- 不自动执行 reload、不自动重启 Shell、不绕过 ExecutionPolicy。
- 不写环境变量/注册表/真实 profile（测试必须隔离 HOME/USERPROFILE）。
- 不新增第二数据库/配置事实来源。
- 不假装“当前会话已生效”；始终区分四层状态。
