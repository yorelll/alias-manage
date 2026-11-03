# TUI 实施计划（下发给 A3 的任务书）

> 状态：规划文档，等待 A2（GUI/CLI 修复）合并后下发给 A3 实现。
> 目标读者：实现 Agent A3、Review Agent B3、功能测试 Agent C3。
> 原则：不是重写 Alias Manager，不为每个 Shell 写不同 TUI；只增加统一入口 `aliasmgr tui`，复用现有核心、CLI 与 Shell 适配器。

## 0. 任务边界

### 必须做

- 新增统一入口：`aliasmgr tui`。
- 一个 TUI 同时管理 Bash / Zsh / PowerShell 5.1 / PowerShell 7 的别名，继续使用现有 Shell 适配器与生成文件。
- 保留现有非交互 CLI 的命令、参数、输出格式、退出码；普通 CLI 命令不得默认进入 TUI。
- 复用现有 SQLite、模型、校验、同步、锁、journal、恢复、导入导出、卸载逻辑。
- 可完成日常任务：搜索/浏览、创建、编辑、重命名、启用/禁用、删除、结构化参数、生成预览、同步、重载命令、诊断。
- 覆盖 Linux 交互终端、Windows Terminal 中的 PowerShell 5.1 / PowerShell 7、Git Bash。
- 提供单元/集成/TestBackend 渲染测试与 CLI 回归测试。

### 明确不做

- 不引入 WebView、浏览器、Node.js、前端构建链。
- 不引入 Tokio 或大型应用框架（除非有不可替代的理由并先说明）。
- 不新增第二套数据库、配置事实来源、后台守护进程、网络服务、遥测。
- 不做动画背景、高频刷新、Nerd Font/emoji/鼠标/系统剪贴板强依赖。
- 不自动执行 reload、自动重启 Shell、绕过 ExecutionPolicy、写标记区块以外的用户配置、删除用户目标文件。
- 不用 `eval` / `Invoke-Expression` / 整行字符串求值。
- 不通过调用自身 CLI 子进程并解析文本来实现 TUI 业务逻辑。

## 1. 已核实的现状（以仓库实际代码为准）

### 工作区

- `crates/aliasmgr-core`：模型、校验、存储、迁移、执行器、Shell 适配器、同步、搜索、transfer、uninstall。
- `crates/aliasmgr-cli`：`lib` + `bin`；`commands` 模块为 `pub`，包含 add/get/list_query/remove/enable/update/rename/find_query/sync/reload_print/doctor/shell_install/shell_uninstall/import/export/uninstall。
- `crates/aliasmgr-gui`：Tauri 2，独立 workspace（`crates/aliasmgr-gui/src-tauri/Cargo.toml` 内有空 `[workspace]`）。
- `crates/aliasmgr-tests`：跨 Shell / 锁 / 基准集成测试。
- 当前 CLI 依赖：`aliasmgr-core`、`clap`、`serde`、`serde_json`、`atty`。**没有任何终端 UI 依赖**。

### 核心 API（已核实）

- `Database`（`crates/aliasmgr-core/src/storage.rs`）：`open`、`open_in_memory`、`open_read_only`、`list_aliases`、`get_alias`、`get_alias_by_name`、`insert_alias`、`insert_aliases_atomic`、`update_alias`、`delete_alias`、`retire_name`、`managed_name_set`、`max_retired_revision`、`shell_states`、`upsert_shell_state`、`transaction`、`overridden_definitions`、`replace_overrides` 等。
- `AliasRecord`（`model.rs`）：`id/name/description/target_type/executable/fixed_args/pass_args/working_directory/environment/shells/enabled/advanced_shell_mode/tags/path_mode/path_origin/created_at/updated_at/record_checksum/revision/restriction`。`revision` 可用于乐观并发。
- `AliasService`（`service.rs`）：目前只有 `list`、`search`、`tag_counts`、`create`、`update`；**没有** delete/rename/enable/get_by_name；且 create/update 已统一调用 `normalize_restriction` + `validate_alias` + `record_checksum`。
- `validate_alias`、`validate_restriction`（`validation.rs`）；`normalize_restriction`、`record_checksum`（`model.rs`）。
- `search`（`search.rs`）：`SearchQuery { query, fuzzy, field, limit, sort, descending, tag_filter }`。
- `SyncCoordinator`（`sync.rs`）：`new(root)`、`apply`、`apply_with_managed_names`、`render_shell`、`recover_pending_operations`；同步锁、备份、journal、原子写、语法检查都在内部。
- `transfer`：`export_json/export_toml`、`read_json/read_toml`、`import_records`、`ConflictStrategy`。
- `uninstall`：`uninstall(root, mode, ...)`。
- `shells::bash/zsh/powershell`：`render` / `render_with_restriction_support` / `render_definition` / `render_cleanup`。

### 已核实的缺口 / 需要最小提取

1. **同步编排在 CLI 层**：`crates/aliasmgr-cli/src/commands.rs::sync` 负责收集引用的 Shell、tombstone、单调 revision、逐 Shell 调 `SyncCoordinator`。GUI 目前没有同步动作（A2 正在补 `sync_aliases`）。TUI 需要结构化结果（每个 Shell 的状态、错误、生成路径），不能解析 CLI 字符串。
   - 建议提取到 core：`sync::sync_all(root) -> SyncReport { revision, results: Vec<ShellSyncResult>, generated_paths }`，CLI/GUI/TUI 共用。A3 开始前先检查 A2 是否已提取；若已提取则直接复用，不得再复制。
2. **诊断有两套**：CLI `commands::doctor` 返回 `Vec<String>`；GUI `commands::doctor_status` 返回结构化 `DoctorFindingDto`。TUI 需要结构化 finding（code/severity/shell/message/action/detail）。
   - 建议把诊断逻辑提取到 core（如 `diagnostics::scan(root) -> Vec<Finding>`），CLI 格式化为文本，GUI/TUI 共用。A2 可能已改动 GUI 诊断；先检查再提取。
3. **Shell loader 安装/卸载在 CLI 层**：`loader_target`、`shell_install`、`shell_uninstall` 含平台 profile 路径解析。TUI 需要“安装 Shell 集成”时，建议提取到 core 的 `shells::integration`，CLI/GUI/TUI 共用。
4. **Service 缺 CRUD**：TUI 需要 delete/rename/enable/get_by_name。建议扩展 `AliasService`（或新增 `AppService`），保留 retire/tombstone 语义，CLI 可逐步复用；不要复制。
5. **reload 命令是纯函数**：`commands::reload_print` 可按 Shell 生成 `source`/`.` 命令；建议提取到 core 或共享模块，TUI/GUI 共用。

### 目标类型语义（TUI 必须正确展示）

- 运行时只有两类特殊行为：`batch` 做额外元字符校验；`change_directory` 生成 `cd`。
- `native_executable`、`python_script`、`powershell_script`、`shell_script`、`java_jar`、`cmdlet` 都直接执行 `executable`，**不会自动拼接解释器**。
- 要运行 `python test.py`：executable=`python`，固定参数=`test.py`；不能把 `python test.py` 填进 executable。
- TUI 详情/编辑应显示“实际 argv 预览”，并给出这条说明。

## 2. 技术方案

### 2.1 代码布局（推荐）

新增 crate：`crates/aliasmgr-tui`。

- 依赖：`aliasmgr-core`；`ratatui`、`crossterm`；`unicode-width`（若 ratatui 已满足则不加）。
- 不依赖 `aliasmgr-cli`（避免层次倒置）；共享业务提取到 core。
- `aliasmgr-cli/Cargo.toml`：

  ```toml
  [features]
  default = ["tui"]
  tui = ["dep:aliasmgr-tui"]

  [dependencies]
  aliasmgr-tui = { path = "../aliasmgr-tui", optional = true }
  ```

- `aliasmgr-cli/src/main.rs`：`Command::Tui` 分支在 `#[cfg(feature = "tui")]` 下调用 `aliasmgr_tui::run(config_dir, ...)`；未启用 feature 时返回清晰错误（不编译 TUI 依赖）。
- `--no-default-features` 构建纯 CLI；默认 release 构建包含 TUI。
- 版本选择：使用与当前 stable toolchain 兼容的稳定版本（先用 `cargo add --dry-run` / 文档确认）；不要随意升级无关依赖。

### 2.2 事件循环与并发

- crossterm `event::poll(timeout)` + `event::read()`，默认 250ms 超时；无事件且无进展时**不重绘**（dirty flag）。
- 同步/诊断/导入等慢操作放到 `std::thread`，通过 `std::sync::mpsc` 把进度/结果送回 UI；同一时间只允许一个写操作。
- 写操作复用现有同步锁；UI 显示“同步中/锁被占用”，不阻塞事件循环。
- 不在 TUI 运行期间 `println!`；错误/日志进入状态栏或日志面板。
- 启动时检查并（在核心支持下）处理未完成 journal；不要自写恢复流程。

### 2.3 数据一致性

- 每次列表刷新重新读数据库，不长期缓存为事实来源。
- 编辑保存前按 `id` 重新读取记录并比较 `revision`：
  - 相同 → 保存；
  - 不同/已删除 → 显示冲突，提供“重新加载 / 放弃本地修改 / 强制覆盖（需确认）”；不得静默覆盖。
- 预览后数据库变化：预览只读；保存时仍按 revision 校验。
- 外部 CLI/GUI 修改：刷新后提示“记录已变化”，保留本地未保存草稿并阻止无提示覆盖。
- 同步后重新读取 shell_state 与生成文件状态。

### 2.4 状态模型（四层，必须区分）

1. 别名已写入数据库（`revision`）。
2. 生成文件已更新（`shell_state.applied_revision` + 文件存在/checksum）。
3. loader 已安装/配置（`shell_state.loader_installed` + 配置路径）。
4. 当前终端是否已加载：**无法可靠确认**，显示“需要手动重载”或“未确认”。

- 保存后：提示“已保存；需要同步”。
- 同步后：显示每个 Shell 结果；提示“仍需要在当前 Shell 执行重载命令”。
- 不自动执行 reload。

## 3. 界面设计

### 3.1 布局

- 顶部（1–2 行）：`Alias Manager`、当前视图（Aliases / Help / Doctor）、配置根（截断）、revision/同步状态。
- 主体：
  - ≥120 列：左侧别名列表（约 60%），右侧详情/编辑（约 40%）。
  - 80–119 列：单栏列表；`Tab`/`Enter` 进入详情；`Esc` 返回。
  - <80 列或 <20 行：清晰提示“终端太小”，只保留退出键，不崩溃。
- 底部（1–2 行）：上下文快捷键 + 状态/错误消息。
- 模态：删除、覆盖、放弃修改、冲突、退出未保存；居中、最大宽度、可滚动。

### 3.2 列表

优先列：名称、目标摘要、Shell、启用状态、标签。空间不足按优先级隐藏标签/Shell。

- 长路径/长参数截断，详情可看完整。
- 中文宽度用 `unicode-width` 计算；截断用 `…` 或 ASCII `...` 回退。
- 空列表显示创建提示；错误状态显示可操作信息。

### 3.3 颜色与字形

- 一种主强调色（青/蓝），中性色为主；成功绿、警告黄、错误红。
- 不靠颜色单独表达状态：同时用文字/符号（如 `[on]`/`[off]`、`!`、`*`）。
- 不使用 emoji / Nerd Font；需要符号时提供 ASCII 回退。
- 支持 `NO_COLOR` / 低颜色终端退化；不假设真彩色。
- 边框克制，优先对齐与留白。

### 3.4 快捷键（建议）

浏览模式：

- `↑/↓` 或 `j/k`：移动
- `Enter`：详情/编辑
- `/`：搜索
- `n`：新建
- `e`：编辑
- `r`：重命名
- `Space`：启用/禁用
- `d`：删除（确认）
- `s`：同步（后台）
- `p`：生成预览
- `D`：Doctor
- `?`：帮助
- `q` / `Ctrl+C`：退出（有未保存修改时确认）
- `Tab`/`Shift+Tab`：列表/详情/字段切换

编辑模式：

- `Tab`/`Shift+Tab`：字段
- `Ctrl+S`：保存
- `Esc`：取消（有未保存修改时确认）
- 参数列表：`a` 添加、`d` 删除、`J/K` 或 `Ctrl+↑/↓` 调整顺序、`Enter` 编辑、空字符串有明确表示
- 文本输入时普通字符只输入，不触发全局快捷键
- 底部始终显示当前上下文可用键

不依赖鼠标；鼠标可作为后续增强，但不得成为必需。

### 3.5 编辑字段

基础：名称、目标类型、executable、固定参数（结构化）、pass_args、Shell、描述、标签。
高级（按需展开）：working_directory、environment（键可见，值默认 `***`）、path_mode、restriction（白名单/黑名单，可后置）。

- 校验调用 core `validate_alias`；前端只做提示，不另立规则。
- 找不到程序：按核心语义作为警告/错误，不阻止另一平台目标保存。
- 保存中禁止重复提交；展示字段级与操作级错误。
- 未保存修改：切换对象/视图/退出必须确认。

## 4. 测试与验收

### 4.1 自动化测试

- 输入状态机：浏览/搜索/编辑/模态/确认；文本输入不触发全局键。
- 过滤、选择、滚动、布局计算（80×24、120×40、40×15、长路径、空列表、CJK）。
- 参数数组：空格、引号、空字符串、CJK；不拆分、不拼接、不把引号当 Shell 语法。
- 编辑校验、未保存确认、保存中防重复、冲突/外部修改。
- 渲染：ratatui `TestBackend` 快照（列表/详情/编辑/错误/小终端/无颜色）。
- 集成：内存 DB + `AliasService`/核心；创建/编辑/删除/重命名/启用；同步；Doctor；revision 冲突。
- 终端生命周期：raw mode/alternate screen 进入退出；panic hook；非 TTY 错误；resize。
- CLI 回归：现有 `aliasmgr-cli`/`aliasmgr-core`/`aliasmgr-tests` 全部通过。

### 4.2 人工/自动验收流程

1. `aliasmgr tui` 正常进入/退出，终端恢复。
2. 创建 `gs` -> `git`，固定参数 `status`；查看参数结构与各 Shell 预览。
3. 保存后显示真实“待同步”状态。
4. 同步，显示每个 Shell 的结果。
5. 查看 reload 命令，明确需要用户手动执行。
6. 另一个 CLI 修改同一记录，TUI 能刷新或提示冲突。
7. 删除/禁用/重命名行为与现有适配器一致。
8. 小终端、长路径、中文、无颜色、非 TTY 均有清晰行为。
9. 退出后光标/输入/画面正常。

### 4.3 轻量性

- 列出新增直接依赖与理由。
- 相同 release 配置下，对比有/无 TUI 的二进制大小。
- 记录可测的启动耗时、空闲 CPU、内存；无法测试的平台/指标明确标注“未验证”。
- 不编造数字。

## 5. 分期与 checkbox

### Phase 0：共享接口提取（A3 开始前）

- [ ] 检查 A2 是否已提取 `sync_all` / 结构化 Doctor / loader 集成；已提取则复用，不重复。
- [ ] 若未提取：在 core 增加共享的 sync orchestration + 结构化诊断 + loader integration，并让 CLI（及 GUI）改用；补测试。
- [ ] 扩展 `AliasService`：`get_by_name`、`delete`（retire+tombstone）、`rename`、`set_enabled`；补并发/冲突测试。
- [ ] 保持 CLI 行为/输出/退出码不变，现有测试全绿。

### Phase 1：TUI 骨架

- [ ] 新 crate + feature 隔离；`aliasmgr tui` 入口；非 TTY 错误。
- [ ] 终端 raw mode/alternate screen 的 RAII 恢复 + panic hook + Ctrl+C。
- [ ] 顶部/列表/底部布局；空列表/加载/错误状态；帮助页。
- [ ] 事件循环 + dirty flag + 250ms poll；无忙循环。

### Phase 2：浏览与 CRUD

- [ ] 搜索、标签过滤、排序、选择、滚动。
- [ ] 详情/编辑视图；名称、目标类型、executable、固定参数、pass_args、Shell、描述、标签。
- [ ] 结构化参数编辑：添加/删除/排序/空字符串/空格/引号/CJK；`{{args}}` 规则与 core 一致。
- [ ] 新建/编辑/重命名/启用/禁用/删除确认。
- [ ] 未保存修改确认；保存中防重复；错误展示。
- [ ] revision 乐观并发/外部修改冲突。

### Phase 3：同步、预览、诊断

- [ ] 生成预览（只读、可滚动、敏感值不泄露）。
- [ ] 后台同步 + 分 Shell 结果 + 生成路径 + reload 命令。
- [ ] 四层状态展示（DB/生成/loader/当前会话未确认）。
- [ ] Doctor 摘要 + 结构化 finding + 可操作项（至少同步）。
- [ ] 同步锁占用/部分失败/恢复提示。

### Phase 4：响应式与细节

- [ ] 120 列双栏 / 窄屏切换；80×24 基本可用；过小提示。
- [ ] CJK 宽度、长路径截断、无颜色退化、ASCII 符号回退。
- [ ] 帮助页列出全部快捷键与目标类型语义。
- [ ] 状态栏消息不覆盖布局、不 `println!`。

### Phase 5：扩展（可选，按优先级）

- [ ] 更完整标签过滤/排序。
- [ ] 批量操作。
- [ ] loader 安装/移除的影响预览与确认。
- [ ] 导入/导出（预览/确认/敏感过滤）。
- [ ] 设置编辑。
- [ ] retain/purge 卸载。

### Phase 6：文档与轻量性

- [ ] README / docs 增加 TUI 使用说明、快捷键、目标类型表、限制。
- [ ] 二进制大小对比、依赖清单、启动/空闲指标（可测项）。
- [ ] 未验证平台/指标清单。

## 6. 风险与待确认

- crossterm 在 Windows PowerShell 5.1 / Git Bash / 不同终端（ConPTY、mintty）下的行为差异；需实测并记录不支持项。
- 中文宽度与终端字体差异；用 `unicode-width`，但不承诺所有字体/终端。
- TUI 依赖增加构建时间/体积；feature 隔离需验证 `--no-default-features`。
- 同步/诊断慢操作与锁竞争；后台线程 + 消息传递，避免阻塞 UI。
- 外部写入与 revision 冲突；必须提示，不得静默覆盖。
- `TestBackend` 只能证明渲染逻辑，不能证明真实终端；真实终端验收由 C3 执行。
- 剪贴板不做首期依赖；至少完整展示命令供终端选取。
- A2 可能同时改 GUI 同步/诊断；A3 必须基于合并后的 HEAD 检查共享接口，避免重复实现或冲突。

## 7. 交付说明（A3 最终报告必须包含）

1. 实际完成的功能（对照 Phase checkbox）。
2. 修改的模块与核心复用方式（明确没有复制业务逻辑）。
3. 构建/启动方法（默认与 `--no-default-features`）。
4. 快捷键表。
5. 新增依赖与轻量性取舍。
6. 实际执行的测试与真实结果（命令、通过数、失败数）。
7. 未完成项、已知限制、待人工验证的平台。
8. commit SHA、分支、工作区状态。

## 8. B3 / C3 关注点

### B3（只 review，不改代码）

- 是否真的复用核心业务，是否在 TUI 复制 DB/Sync/Doctor/Uninstall 逻辑。
- revision/外部修改/锁/部分同步失败/恢复是否正确。
- 参数数组是否保持 argv 边界，是否有字符串拼接/求值/引号解释。
- 敏感环境变量是否泄露到默认详情/预览/错误/日志/复制内容。
- 终端 raw mode/alternate screen/panic/非 TTY/resize 是否安全。
- 快捷键状态机、未保存确认、删除确认、文本输入是否冲突。
- 布局在小终端/长路径/CJK/无颜色下是否可读。
- CLI 行为/输出/退出码是否保持不变；feature 隔离是否有效。

### C3（详细功能测试 + 改进建议）

- 四终端实测：Windows PS5.1、pwsh7、Linux Bash、Zsh；非 TTY、resize、Ctrl+C、退出恢复。
- 执行人工验收 9 步，保存完整证据（命令、输出、截图/录屏路径、退出码、终端类型/版本、二进制 SHA256）。
- 评估并给出可改进项：排版、布局、字体/字形、功能、快捷键、用户交互、显示颜色、UI 一致性、TUI 与 CLI 数据同步、两者互相影响。
- 每项结果使用 `PASS | FAIL | BLOCKED | EXPECTED-LIMITATION | NOT-APPLICABLE`，不得编造。
- 测试后清理临时配置根/进程/终端状态，零残留。

## 9. Round 1：C3 UX 改进（A3 → B3 → C3）

基于 C3 的真实 PTY 测试与改进建议，优先完成以下可验证项：

- [ ] **按别名预览**：预览只渲染当前选中的别名定义，不因其他别名目标缺失而整体失败；保留整文件预览作为可选/回退，并明确标示。
- [ ] **失败 Shell 重试**：部分 Shell 同步失败后，可只重试失败项，显示每次结果；不重复成功项。
- [ ] **自适应 footer**：窄终端/80×24 下 footer 折行或按优先级隐藏，不出现 `d …` 截断；快捷键与状态至少一项完整可见。
- [ ] **真正的 ASCII 字形集**：`ALIASMGR_TUI_ASCII=1` 时边框/分隔符使用 `+ - |`，不依赖 Unicode 制表符；无颜色时仍可读。
- [ ] **焦点指示**：列表/详情/编辑字段/模态明确显示当前焦点，不只靠颜色。
- [ ] **滚动位置指示**：列表/预览/Doctor 显示 `x/y` 或等价位置，长内容可判断。
- [ ] **刷新时间与手动刷新**：状态栏显示 last refresh；提供手动刷新键（如 `R`）并记录外部变更。
- [ ] **重命名实时校验**：输入时按 core 规则提示非法/保留/冲突名称，保存前阻止。
- [ ] **删除影响列表**：确认框列出将 retire 的 Shell/名称、tombstone 影响与目标文件不会删除。
- [ ] **字段级冲突 diff**：外部修改时显示哪些字段变化（旧值/新值），支持 reload/discard/overwrite。
- [ ] **Esc 清除已应用筛选**：搜索/标签/排序可按 Esc 清除并恢复列表。
- [ ] 为以上各项补齐确定性测试（输入状态、渲染、无颜色/ASCII、窄终端、冲突/重试）。

验收：B3 无 blocking/confirmed 问题；C3 在 Linux Bash/Zsh 与 Windows PS5.1/PS7 真实 PTY 中验证，并给出剩余建议（区分缺陷与建议）。

## 10. Round 2：Phase 5 扩展（A3 → B3 → C3）

- [ ] **更完整标签过滤与排序**：标签列表/多选（AND/OR 语义与 CLI 一致）、按标签/更新时间/创建时间/启用状态排序、descending。
- [ ] **批量操作**：多选、批量启停/删除/加标签/去标签；删除确认列出影响；逐项结果与部分失败。
- [ ] **loader 安装/移除**：影响预览（目标 profile/RC、将写入/移除的标记块、备份与保留策略）、确认、结果；复用 `integration`，不手写路径逻辑。
- [ ] **导入/导出**：文件路径输入、预览/确认/取消、敏感环境变量过滤、冲突策略与结果；复用 `transfer`。
- [ ] **设置编辑**：默认 Shell、备份/日志保留、允许相对路径、语言等；复用 `AppConfig`/`config_save` 语义；配置目录仍固定。
- [ ] **卸载**：retain/purge 影响预览与二次确认；复用 core `uninstall`；不删除用户目标文件。
- [ ] 以上操作的后台执行、锁、错误码、i18n/文案、部分失败与退出后的终端恢复测试。
- [ ] 更新 `docs/tui.md`、README、architecture/limitations；记录未实现项。

验收：B3 无 blocking/confirmed 问题；C3 四终端真实 PTY + CLI/TUI 数据同步验证，输出通过/失败/阻塞与改进建议。

## 11. Round 2 执行顺序（已授权）

按以下顺序逐项实施；每一项都走完整的 A3 → B3 → C3 循环，C3 无 FAIL 后进入下一项：

1. **R2.1 导入/导出 + 设置编辑**（复用 `transfer` / `AppConfig`，低风险）
2. **R2.2 loader 安装/移除**（复用 `integration`，解决“别名为什么不生效”）
3. **R2.3 标签/排序增强 + 批量操作**
4. **R2.4 retain/purge 卸载**（破坏性最强，最后单独确认）

每一项的验收：
- 复用 core，不在 TUI 复制业务规则；一个 SQLite 数据库；CLI 行为/退出码不变。
- 慢操作后台线程 + mpsc；写操作一次一个，复用锁；revision 冲突不静默覆盖。
- 正常/边界/错误/回归测试；B3 无 blocking/confirmed；C3 四终端真实 PTY 通过。
- 敏感环境变量不泄露；不自动 reload/重启/改环境变量/注册表。

## 12. R2.1b：Footer 重排与全局语言模式（方案 B）

在 R2.1 通过后、R2.2（loader）之前实施。

### Footer
- 键位强调：快捷键用主强调色 + 粗体，标签用中性/dim 色；不只靠颜色区分。
- 分组分隔：只在逻辑组之间使用 dim `│`，组内用 `·`/空格；不对每个命令都加竖线。
- 分组建议：移动/浏览；编辑/操作（new/edit/rename/toggle/delete）；同步/查看（sync/preview/doctor）；帮助/退出。
- ASCII 回退：`ALIASMGR_TUI_ASCII=1` 时 `│`→`|`、`·`→`-`。
- 自适应折行：80×24 优先保留高优先级组，低优先级整项丢弃，不截断成 `d …`；120×40 显示完整组。
- 破坏性操作（delete）额外用 `!`/文字标记，不依赖红色。
- 测试：80×24、120×40、无颜色、ASCII、折行/分组、无截断 token。

### 全局语言模式（zh/en，默认 en，方案 B）
- 唯一事实来源：共享 `config.toml` 的 `runtime.language`；CLI/TUI/GUI 通过同一配置根读写。
- 默认英文：core/GUI/TUI/CLI 默认 `en`；旧配置缺字段按 `en` 解释（迁移测试）。
- CLI：
  - `aliasmgr language` 显示当前语言；`aliasmgr language en|zh` 设置并保存；非法值退出码 2。
  - CLI 用户消息/错误跟随语言。
  - **方案 B：clap `--help`/子命令 help/参数说明也本地化**；解析前预读 `--config-dir`/`ALIASMGR_CONFIG_DIR` 以选择语言包；`aliasmgr help language`、`language -h`、`language --help` 均提供。
  - 不写环境变量/注册表。
- TUI：zh/en 完整语言包；设置页/快捷键切换；即时重绘并写回共享配置；外部语言变更可感知/刷新；所有用户可见文案走统一 `t(key)`。
- GUI：沿用现有 zh/en 语言包，默认改 en；切换写共享配置；获得焦点/刷新时重读 CLI/TUI 的修改。
- 一致性测试：CLI 设置→TUI/GUI 读取；TUI 设置→CLI/GUI；GUI 设置→CLI/TUI；同一配置根；无效值；默认 en；旧配置迁移；外部变更刷新。
- 文档：README、docs/tui.md、docs/cli-commands.md、architecture/limitations 更新；说明语言包可以不同但模式共享。

## 13. R2.0：保存即完成（auto-sync）与首次 Shell 集成（待最终确认后实施）

目标：用户只关心“在哪些 Shell 里使用别名”。保存/删除/启停/重命名后自动同步；Shell 集成首次引导、后续只做失效检测。**不新增 `setup` 命令**；GUI/TUI 首次向导，CLI 用交互式 `doctor`。

### 13.1 共享状态与配置

- `config.toml` 增加用户意图：
  ```toml
  [runtime]
  auto_sync = true

  [shells]
  enabled = ["powershell5", "powershell7"]

  [shells.paths.powershell7]
  executable = "C:\Program Files\PowerShell\7\pwsh.exe"
  profile = "C:\Users\<user>\Documents\PowerShell\Microsoft.PowerShell_profile.ps1"
  ```
- `enabled` 是用户意图；loader 有效性每次启动/doctor 重新检测，不持久化为事实。
- 保留现有 profile 路径覆盖；补充可选 `executable` 用于非标准路径检测。
- 已有 loader 的 Shell 迁移为 enabled；其余空，首次引导提示。
- 所有入口共享同一配置，原子写；启动/刷新重读。

### 13.2 auto-sync

- CLI `add/update/remove/enable/disable/rename` 成功后自动同步受影响的 Shell；`--no-sync` 逃生。
- TUI/GUI 保存/删除/启停后台自动 sync；`s` 保留为手动修复/重试。
- 只同步目标 Shell，不为未 enabled 的 Shell 生成文件。
- loader 缺失不阻止 sync，但提示“新终端不会自动加载”。
- 失败语义：明确显示“已保存到数据库；以下 Shell 同步失败：…”；默认返回 `PartialSync`（exit 10）以便脚本感知，或提供 `--sync=strict|auto|off`（默认 strict）。不得吞掉失败只显示绿色成功。
- 批量/导入后只 sync 一次；导入预览不 sync。

### 13.3 GUI/TUI 首次引导与后续检测

- 首次打开列出全部 4 个 Shell（不是只列检测到的），状态：detected / not detected / enabled / loader valid。
- detected + 勾选：显示 profile/RC 路径与标记块预览；确认后安装 loader + auto-sync。用户无感。
- not detected + 勾选：三选项：
  1. 安装该 Shell 并加入 PATH，重启软件；
  2. 已安装但路径非标准：填写 executable 与 profile/RC 路径，无需重启；
  3. 取消该 Shell enable。
- 未勾选：写入 enabled=false，后续不主动骚扰；设置页可重新启用。
- 后续每次启动只检查 enabled Shell：loader 无效时提示“X 集成已失效，请在设置中重新启用”，提供 重新启用 / 暂时忽略 / 禁用；不每次弹全屏向导。
- 新检测到未 enabled 的 Shell 可提示一次，不强制。
- 添加/编辑别名选择 Shell：显示 enabled/disabled/not installed；选择未 enabled 的 Shell 时保存拦截，进入同一 enable 流程；拒绝启用则保留草稿、不静默保存无效别名。

### 13.4 CLI doctor / add 预检

- `aliasmgr doctor`：stdin/stdout 均为 TTY 时进入交互式 Shell 集成界面；非 TTY 保持现有文本输出（可加 `--format json`），不进入 raw mode、不等待输入。
- 交互界面：spinner + 每个 Shell 的真实检测状态（不伪造百分比）；↑/↓ 选择，Space enable，Enter 应用，r rescan，q 退出。
- detected：安装 loader + sync + 显示 reload 命令。
- not detected：同样三选项；填写路径时分别输入 executable 与 profile/RC（给出默认值/“使用默认路径”），安装前预览。
- `help`/`doctor` 文本指导用户完成启用。
- `add/update` 写库前检查目标 Shell 的 enabled + loader valid；无效则拒绝保存并提示 `aliasmgr doctor`；提供 `--allow-missing-loader` 显式逃生（文档说明该 Shell 暂不生效）。
- 退出码沿用现有映射（如 ShellNotInstalled=12）；不得破坏脚本/CI。

### 13.5 检测边界

- 增强检测：PATH + 已知路径（PS5 系统路径、PS7 常见安装路径、Git Bash、MSYS2/Cygwin Zsh）；PS7/WSL 单独标记。
- 始终列出全部 4 个 Shell；允许手动 executable/profile；Rescan。
- 无法保证 100% 自动发现；目标 Shell 未 enabled/loader 无效时明确警告，不假装可用。
- WSL 的 bash 不自动写 Windows profile，提示在对应发行版内安装。
- 后装 Bash/Zsh：下次启动检测/提示，或 `doctor`/设置页启用；无需 setup。

### 13.6 风险

- 启动弹窗骚扰 → 持久化 enabled/disabled；失效只提示一次，可忽略/禁用。
- 用户填错 profile → 安装前预览、备份/保留、只改标记块。
- CLI 脚本被交互破坏 → 严格 TTY 判定；非 TTY 保持文本/JSON。
- auto-sync 失败被吞 → 明确状态与退出码；`--no-sync`/`--sync` 选项。
- 多入口并发改配置 → 共享配置 + 原子写 + 启动/刷新重读；不引入第二事实来源。

### 13.7 测试与验收

- 配置迁移/enabled 持久化/未知字段保留/原子写。
- auto-sync 成功、部分失败、锁占用、`--no-sync`/`--sync` 语义、CLI 退出码。
- GUI/TUI 首次引导（detected/not detected/三选项/取消）、后续失效提示、添加别名预检、拒绝保存保留草稿。
- CLI doctor TTY/非 TTY、路径填写、rescan、安装后 sync、reload 展示。
- 检测覆盖 PS5/PS7 常见路径、Git Bash/WSL 标记、后装 Shell。
- 真实 PTY 四终端 + 证据；无环境变量/注册表/真实 profile 的测试污染。
