# 跨平台别名管理器实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development`（推荐）或 `superpowers:executing-plans` 执行本计划。所有步骤使用复选框跟踪，并在每个独立任务后运行对应测试。
>
> **验证方式：本项目采用 CI 驱动开发。本地没有 Rust 工具链（已实测：无 `cargo`/`rustc`/`rustup`），所有 `cargo` 命令只在 GitHub Actions 上执行，禁止在本地尝试。** 任务清单中带 `【CI】` 标记的条目表示“该命令由 CI 执行并通过”，带 `【本地】` 的条目才在本机执行（仅 `git`、`gh`、`npm`、`python`）。`gh` 不在 PATH，必须用绝对路径 `"D:/Program Files/GitHub CLI/gh.exe"`。执行任何任务前必须先读 §11，其中的执行位置约定（§11.1.1、§11.1.2）、授权边界（§11.2）与迭代规则（§11.3）是硬性约束。

**目标：** 构建一个支持 Linux 与 Windows 的跨平台命令别名管理器，以统一别名模型、安全的结构化参数透传、Shell 适配器和可回滚配置同步为核心。**跨平台 GUI 与 Linux CLI 同为基础需求**，两者共享同一核心库。

**架构：** 使用 Rust workspace 实现可复用核心库；SQLite 是别名配置的唯一事实来源，Bash、Zsh、PowerShell 5.1 和 PowerShell 7 通过独立适配器生成派生脚本。用户的 RC/Profile 文件只保留带明确标记的加载块，实际定义写入应用管理的独立生成文件，并通过 revision、operation journal、锁、备份、语法检查、临时文件和原子替换完成同步。

**术语约定：** `alias` 是 Linux/Bash/Zsh/Fish 中的命令机制；Bash、Zsh、Fish 和 PowerShell 都是 Shell/命令解释器，不是终端类型或“中断类型”。终端是承载 Shell 的程序，例如 Windows Terminal、GNOME Terminal 或 Konsole。

**技术栈：** Rust stable、Cargo workspace、Serde、SQLite（`rusqlite`，必须启用 `features = ["bundled"]`，避免依赖 Windows 系统 SQLite）、`clap`、`uuid`、`chrono`、`thiserror`、`tracing` + `tracing-appender`、文件锁库（`fs4`，需在 Linux/Windows 均验证；`fd-lock` 作为备选）、Tauri 2（Linux 侧依赖 `webkit2gtk-4.1`，最低支持发行版范围必须在 M0 确认并写入文档）、TypeScript、Vitest/Playwright。

**语言约定：** 面向用户的 CLI/GUI 文案与文档默认中文；错误枚举名、日志字段、JSON 字段名、commit message 使用英文。MVP 不引入运行时 i18n 框架，但所有用户可见文案必须集中在单一文案模块，便于后续接入。

---

## 0. 范围、原则与完成定义

### 0.1 首期 MVP 范围

首期必须实现：

- Rust 核心库和 SQLite 存储（含迁移执行器与版本兼容检查）；
- Linux CLI；
- **跨平台 GUI（Linux + Windows）**：主列表、搜索、新增/编辑向导、详情、诊断、设置与卸载选项；
- Bash、Zsh、Windows PowerShell 5.1、PowerShell 7；
- 别名增删改查、启用/禁用、精确查找、模糊查找和排序；
- 结构化可执行文件与固定参数；
- 工作目录、环境变量、标签（见 §0.2 决策说明）；
- 用户参数透传：Bash/Zsh 使用 `"$@"`，PowerShell 使用 `@args`；支持 `{{args}}` 占位符控制透传位置；
- 名称抢占处理：定义前清理同名 alias/function（见 §3.4）；
- 默认优先生成函数包装器；只有满足 §3.2 全部“简单条件”时，才使用原生 alias/`Set-Alias`；
- 独立生成文件和 RC/Profile 加载块；
- 配置备份、语法检查、文件锁、原子替换和失败回滚；
- 每 Shell 独立的同步状态跟踪（`shell_state`）；
- `doctor` 诊断（含 `.bashrc` 生效性、PowerShell ExecutionPolicy、派生文件过期）；
- JSON 和表格输出、稳定退出码表；
- 卸载时保留或删除托管别名的清理流程；
- 完整的单元测试和 Linux/Windows Shell 集成测试；
- 可用的 GitHub Actions 验证流水线（§11），作为本项目唯一的跨平台验证手段。

### 0.2 范围决策与后续阶段

**纳入 MVP 的范围决策：**

- **GUI 是基础需求，不是后续阶段。** 原始需求要求 Linux 与 Windows 均提供可视化界面，因此 M5 的 Tauri GUI 属于 MVP 交付物；Windows 用户必须在首个版本就拿到界面。
- **工作目录、环境变量、标签纳入 MVP。** 这三项在数据模型、生成规则、搜索字段中已被广泛依赖，拆分反而增加分支成本；`ChangeDirectory` 类别名也是高频需求。原 Task 22 因此并入阶段一/二，不再作为扩展任务。

**推迟到后续阶段：** Fish、POSIX sh 兼容模式、配置历史、现有 Shell 别名安全导入、WSL、CMD、Git Bash、Nushell、插件系统、企业策略和脚本签名不进入 MVP，但核心接口必须为适配器和执行器扩展保留边界。

**明确的非目标（不做，并在文档中说明）：**

- 别名引用别名：`ll -> ls -l` 且 `ls` 也是托管别名时，不做递归解析，按字面执行由 Shell 自行决定；
- 非交互式 Shell 中使用别名：Bash/Zsh 的 alias 不导出、函数需 `export -f`，托管别名只保证在交互式 Shell 生效；
- 自动修改父 Shell 会话状态。

### 0.3 不可违反的原则

1. 普通模式禁止使用 Bash `eval`、PowerShell `Invoke-Expression` 或等价的整行字符串求值。
2. 用户参数必须以参数数组表示，在最后生成目标 Shell 的安全调用代码。
3. 默认优先生成函数包装器；只有满足 §3.2 全部“简单条件”时，才使用原生 alias/`Set-Alias`。
4. 不直接把每条别名写入用户的 `.bashrc`、`.zshrc` 或 PowerShell Profile。
5. SQLite 数据库是唯一事实来源；生成 Shell 脚本是可重建的派生文件，不反向作为主配置。
6. 修改用户配置前必须创建备份，并且只处理 Alias Manager 自己的标记块。
7. 正式替换生成文件前必须完成语法检查；检查失败不得覆盖正式文件。
8. 每次数据库或生成文件同步都带 revision，并通过 operation journal 支持崩溃恢复。
9. 默认只管理当前用户配置，不自动提权、不修改系统级配置。
10. 相对路径只能在用户显式启用后保存；GUI 选择的脚本和可执行文件默认保存为绝对路径。
11. 卸载保留别名时，生成脚本不得依赖已卸载的 `aliasmgr` 程序；卸载流程不得删除别名引用的目标文件。
12. 导入配置按可执行代码处理，必须预览、验证、生成安全报告并经用户确认后才允许同步。
13. 删除或强制覆盖后必须提供当前会话清理/恢复提示，不能假定重新加载配置会恢复被覆盖的用户定义。
14. PowerShell 5.1 生成的 `.ps1` 使用 UTF-8 BOM，避免中文路径或注释乱码。
15. 已打开的父 Shell 无法被子进程自动更新，CLI 和 GUI 必须明确提示重新加载命令。
16. 生成函数前必须先清理同名 alias（Bash/Zsh）或同名 alias 与 function（PowerShell）；否则由于名称解析优先级，函数定义会失败或永不生效。见 §3.4。
17. 托管名称清理清单必须包含退役名称（tombstone），仅依赖“当前托管名称”无法清除已删除别名的残留定义。
18. 语法检查通过不等于别名生效。任何“已支持”“已生效”的结论都必须有对应测试或诊断证据。
19. 加载块只追加到用户配置文件末尾，不与用户的插件管理器竞争名称；发现后置覆盖时只报告，不擅自调整用户配置顺序。

### 0.3.1 已知的原则冲突

原则 1（禁止字符串求值）与 §3.3 高级 Shell 模式（允许管道、重定向、逻辑连接）在功能上是对立的：任何真正执行管道/重定向的实现都必然依赖 `bash -c`、`Invoke-Expression` 或等价机制。本计划正面承认该冲突，处理方式为：**MVP 阶段 `validation` 直接拒绝 `advanced_shell_mode = true` 与 `TargetType::RawShellCommand`**，枚举与字段保留仅用于前向兼容与导入检测。若后续启用高级模式，必须以独立的、显式开关控制的执行路径实现，并在文档中标注它不受原则 1 保护。

### 0.4 MVP 完成定义

MVP 只有在以下条件全部满足时才算完成：

- Linux 能识别当前父进程 Shell、默认 Shell 和已安装 Shell，并允许显式指定；
- Bash/Zsh/PowerShell 5.1/PowerShell 7 均能生成并加载别名；
- `cm = python3 copymv.py` 后执行 `cm cp "file name.txt"` 时目标程序收到两个完整参数；
- 目标名称已存在同名 alias（含 PowerShell 内置 alias，如 `ls`、`cp`、`gc`）时，托管定义仍然生效；
- 删除别名后数据库和生成文件都没有该定义，重新加载后当前会话残留定义被清除，用户手工配置不变；
- 语法错误、并发修改或写入失败时可以恢复到上一个有效状态；多 Shell 部分失败时 `shell_state` 能准确反映哪个 Shell 过期；
- 导入/导出不默认携带密码、Token 或 API Key；
- 普通模式的生成代码中不存在 `eval` 和 `Invoke-Expression`；`advanced_shell_mode` 被 validation 拒绝；
- 卸载保留模式下，生成脚本脱离 Alias Manager 仍可执行；
- GUI 在 Linux 与 Windows 上均可完成增删改查、搜索、同步、诊断与卸载设置；
- CI 在 Linux 和 Windows 上通过核心单元测试、适配器测试、CLI 测试和 GUI 测试；
- `ci.yml`、`integration.yml`、`release.yml` 三个 workflow 均能成功运行，且 CI 测试全程使用隔离的临时 HOME/配置目录，未修改 runner 真实用户配置（§11.7）。

---

# 第一部分：架构规格

## 1. 总体架构

### 1.1 Workspace 结构

从空仓库初始化以下结构：

```text
alias-manage/
├── Cargo.toml
├── Cargo.lock
├── plan.md
├── crates/
│   ├── aliasmgr-core/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── model.rs
│   │       ├── error.rs
│   │       ├── validation.rs
│   │       ├── search.rs
│   │       ├── executor.rs
│   │       ├── storage.rs
│   │       ├── config.rs
│   │       ├── lock.rs
│   │       ├── detection.rs
│   │       ├── transfer.rs
│   │       ├── uninstall.rs
│   │       ├── sync.rs
│   │       ├── shell.rs
│   │       ├── shells/
│   │       │   ├── mod.rs
│   │       │   ├── bash.rs
│   │       │   ├── zsh.rs
│   │       │   ├── powershell.rs
│   │       │   └── common.rs
│   │       └── migrations/
│   │           ├── mod.rs
│   │           └── 0001_initial.sql
│   ├── aliasmgr-cli/
│   │   ├── Cargo.toml
│   │   ├── src/
│   │   │   ├── main.rs
│   │   │   ├── cli.rs
│   │   │   ├── commands.rs
│   │   │   ├── exit_code.rs
│   │   │   ├── messages.rs
│   │   │   └── output.rs
│   │   └── tests/
│   │       ├── parse.rs
│   │       ├── crud.rs
│   │       ├── diagnostics.rs
│   │       ├── transfer.rs
│   │       └── uninstall.rs
│   ├── aliasmgr-gui/
│   │   ├── Cargo.toml
│   │   ├── src-tauri/
│   │   │   └── src/
│   │   │       ├── main.rs
│   │   │       └── commands.rs
│   │   └── ui/
│   │       ├── package.json
│   │       ├── src/
│   │       └── tests/
│   └── aliasmgr-tests/
│       ├── Cargo.toml
│       ├── fixtures/
│       │   ├── argument-dumper.py
│       │   └── argument-dumper.ps1
│       └── tests/
│           ├── bash.rs
│           ├── zsh.rs
│           └── powershell.rs
└── docs/
    ├── architecture.md
    ├── security.md
    ├── testing.md
    ├── exit-codes.md
    └── limitations.md
```

**测试布局约束：** Cargo 不会编译 virtual workspace 根目录下的 `tests/`。所有集成测试必须位于某个 crate 的 `tests/` 目录内。CLI 端到端测试放在 `crates/aliasmgr-cli/tests/`；需要真实 Shell 的跨 crate 集成测试放在专用的 `crates/aliasmgr-tests/`（该 crate 不发布，仅含 `tests/` 与 `fixtures/`）。运行方式为 `cargo test -p aliasmgr-cli --test crud`、`cargo test -p aliasmgr-tests --test bash`。

### 1.2 模块依赖

```text
aliasmgr-cli ───────┐
aliasmgr-gui ────────┼──> aliasmgr-core
                    │       ├── model / validation / search
                    │       ├── storage
                    │       ├── executor
                    │       ├── shell adapters
                    │       └── sync / backup / recovery
```

`aliasmgr-core` 不依赖 GUI 或 CLI。CLI 和 GUI 只能通过核心库的公开接口读写数据、生成脚本和执行同步；不得各自实现一套参数解析、Shell 转义或配置修改逻辑。

### 1.3 核心公开接口

核心库至少提供以下稳定边界：

```rust
pub trait ShellAdapter {
    fn kind(&self) -> ShellKind;
    fn detect(&self, ctx: &DetectionContext) -> DetectionResult;
    fn locate_config(&self) -> Result<ShellConfig, AliasError>;
    fn validate_name(&self, name: &str) -> Result<(), AliasError>;
    fn render_entry(&self, alias: &AliasRecord) -> Result<String, AliasError>;
    /// 生成定义前清理同名 alias/function 的代码片段，见 §3.4。
    fn render_preemption(&self, name: &str) -> String;
    /// 生成 reload 前清理托管名称（含 tombstone）的代码片段，见 §5.1.1。
    fn render_cleanup(&self, managed: &ManagedNameSet) -> String;
    fn render_loader(&self, generated_path: &Path) -> String;
    fn install_loader(&self, config: &Path) -> Result<SyncReceipt, AliasError>;
    fn remove_loader(&self, config: &Path) -> Result<SyncReceipt, AliasError>;
    fn syntax_check(&self, script: &Path) -> Result<(), AliasError>;
    fn detect_conflicts(&self, name: &str) -> Result<Vec<Conflict>, AliasError>;
}

pub trait Executor {
    fn target_type(&self) -> TargetType;
    fn validate(&self, alias: &AliasRecord) -> Result<(), AliasError>;
    fn render_argv(&self, alias: &AliasRecord) -> Result<Vec<String>, AliasError>;
}
```

实际实现可以增加参数，但必须保持“数据模型、执行器、Shell 适配器、同步器”之间的职责分离。

---

## 2. 数据模型与 SQLite

### 2.1 `AliasRecord`

```rust
pub struct AliasRecord {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub target_type: TargetType,
    pub executable: String,
    pub fixed_args: Vec<String>,
    pub pass_args: bool,
    pub working_directory: Option<String>,
    pub environment: BTreeMap<String, String>,
    pub shells: Vec<ShellKind>,
    pub enabled: bool,
    pub advanced_shell_mode: bool,
    pub tags: Vec<String>,
    pub path_mode: PathMode,
    pub path_origin: PathOrigin,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub record_checksum: String,
    pub revision: i64,
}

pub enum PathMode {
    Absolute,
    RelativeToWorkingDirectory,
}

pub enum PathOrigin {
    GuiFilePicker,
    CliArgument,
    Import,
}
```

`fixed_args` 必须是参数数组，不保存未经解析的完整命令行。

**参数占位符：** `fixed_args` 中允许出现且最多出现一次字面元素 `{{args}}`，表示用户参数的插入位置。规则：

- 未出现 `{{args}}` 且 `pass_args = true`：用户参数追加到末尾（默认行为，等价于末尾占位）；
- 出现 `{{args}}`：该元素被整体替换为用户参数序列，可实现 `cmd <args> --verbose` 这类中间插入；
- 出现 `{{args}}` 但 `pass_args = false`：validation 报错 `InvalidArgTemplate`；
- 出现多个 `{{args}}`：validation 报错 `InvalidArgTemplate`（避免参数重复展开的语义歧义）；
- 需要传递字面量 `{{args}}` 时使用 `{{{{args}}}}` 转义；
- 占位符只做**数组元素级**替换，不做元素内部的字符串插值，以保持参数数组边界不被破坏。

Bash/Zsh 的中间插入使用 `command exe pre... "$@" post...`；PowerShell 使用 `& exe pre... @args post...`。两者都不引入字符串拼接。

**`advanced_shell_mode`：** 字段与 `TargetType::RawShellCommand` 仅为前向兼容与导入检测保留。MVP 中 `validate_alias` 对 `advanced_shell_mode = true` 或 `target_type = RawShellCommand` 一律返回 `AdvancedModeUnsupported`，既不保存也不执行；导入时该类记录标记为 unsupported 并跳过（见 §8.3）。理由见 §0.3.1。

`sort_index` 已从模型中移除：没有对应的重排序命令，排序完全由 §8.1 的查询期排序字段决定。

### 2.2 枚举

```rust
pub enum TargetType {
    NativeExecutable,
    PythonScript,
    PowerShellScript,
    Batch,
    ShellScript,
    JavaJar,
    Cmdlet,
    ChangeDirectory,
    RawShellCommand,
}

pub enum ShellKind {
    Bash,
    Zsh,
    PowerShell5,
    PowerShell7,
    Fish,
    PosixSh,
}
```

### 2.3 SQLite 表

`0001_initial.sql` 创建：

```sql
CREATE TABLE aliases (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    name_folded TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    target_type TEXT NOT NULL,
    executable TEXT NOT NULL,
    fixed_args_json TEXT NOT NULL,
    pass_args INTEGER NOT NULL DEFAULT 1,
    working_directory TEXT,
    environment_json TEXT NOT NULL DEFAULT '{}',
    shells_json TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    advanced_shell_mode INTEGER NOT NULL DEFAULT 0,
    tags_json TEXT NOT NULL DEFAULT '[]',
    path_mode TEXT NOT NULL DEFAULT 'absolute',
    path_origin TEXT NOT NULL DEFAULT 'cli_argument',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    record_checksum TEXT NOT NULL,
    revision INTEGER NOT NULL DEFAULT 1
);

CREATE UNIQUE INDEX aliases_name_unique ON aliases(name COLLATE BINARY);
CREATE INDEX aliases_name_folded_idx ON aliases(name_folded);
CREATE INDEX aliases_updated_at_idx ON aliases(updated_at);
CREATE INDEX aliases_enabled_idx ON aliases(enabled);

CREATE TABLE operation_journal (
    id TEXT PRIMARY KEY NOT NULL,
    operation TEXT NOT NULL,
    state TEXT NOT NULL,
    alias_id TEXT,
    revision_from INTEGER NOT NULL,
    revision_to INTEGER NOT NULL,
    backups_json TEXT NOT NULL DEFAULT '[]',
    generated_paths_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    completed_at TEXT
);

CREATE TABLE shell_state (
    shell TEXT PRIMARY KEY NOT NULL,
    applied_revision INTEGER NOT NULL DEFAULT 0,
    file_checksum TEXT NOT NULL DEFAULT '',
    loader_installed INTEGER NOT NULL DEFAULT 0,
    config_path TEXT,
    last_sync_at TEXT,
    status TEXT NOT NULL DEFAULT 'unknown',
    last_error TEXT
);

CREATE TABLE retired_names (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    shell TEXT NOT NULL,
    definition_kind TEXT NOT NULL,
    retired_at_revision INTEGER NOT NULL,
    retired_at TEXT NOT NULL
);

CREATE INDEX retired_names_shell_idx ON retired_names(shell, retired_at_revision);

CREATE TABLE overridden_definitions (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    shell TEXT NOT NULL,
    definition_kind TEXT NOT NULL,
    original_definition TEXT,
    recoverable INTEGER NOT NULL DEFAULT 0,
    captured_at TEXT NOT NULL
);
```

SQLite 事务负责别名记录、`shell_state`、`retired_names` 和操作日志；生成文件、用户配置和备份由同步器负责。数据库始终是唯一事实来源，生成文件缺失或损坏时只能从数据库重建，不能反向覆盖数据库主配置。

**名称唯一性模型（决策）：** 别名名称**全局唯一**，不区分 Shell。即同一个名称不能在 Bash 和 PowerShell 下指向不同目标；需要跨 Shell 差异化时应使用不同名称。理由：跨 Shell 同名不同义会让搜索、诊断、冲突提示与会话清理的语义全面复杂化，而收益有限。该约束必须在 CLI/GUI 的冲突提示中明确表述。

**大小写规则：** `name` 列使用 `COLLATE BINARY`，保留用户输入的原始大小写，Bash/Zsh 按大小写敏感处理。但 PowerShell 名称解析大小写不敏感，因此：

- `name_folded` 存储 `name.to_lowercase()`；
- 当新增/更新的别名 `shells` 中包含任一 PowerShell 变体时，必须额外检查是否存在 `name_folded` 相同但 `name` 不同、且同样面向 PowerShell 的记录，命中则返回 `AliasConflict` 并说明是大小写冲突；
- 纯 Bash/Zsh 别名之间允许 `cm` 与 `CM` 共存。

**checksum 语义区分：**

- `aliases.record_checksum`：单条别名记录内容的校验和，用于检测记录级篡改与导入去重；
- `shell_state.file_checksum`：**生成文件整体内容**的校验和，是 §5.3 手工修改检测的唯一依据。

两者不可混用。生成文件头部写入 `revision` 与该文件的 `file_checksum`。

### 2.3.1 迁移与版本兼容

- 迁移文件按 `NNNN_name.sql` 递增命名，由 `migrations/mod.rs` 中的迁移执行器在单个事务内顺序执行；
- 版本记录使用 `PRAGMA user_version`（值等于已应用的最大迁移编号），另建 `schema_migrations(version, applied_at, checksum)` 表记录每个迁移文件的内容 checksum，防止已应用迁移被事后修改；
- **向下兼容检查：** 若 `user_version` 大于当前程序已知的最大迁移编号，必须拒绝打开数据库并返回 `SchemaTooNew`，提示用户升级程序；禁止在此情况下继续读写；
- 迁移前自动创建 `aliases.db` 的完整文件备份到 `backups/`，迁移失败即恢复；
- 连接参数：`PRAGMA journal_mode = WAL`、`PRAGMA foreign_keys = ON`、`PRAGMA busy_timeout = 5000`、`PRAGMA synchronous = FULL`；
- **网络/特殊文件系统检测：** WAL 与 `flock` 在 NFS、CIFS/SMB、部分 WSL 挂载点上不可靠。启动时检测配置目录所在文件系统类型，命中不可靠列表时降级为 `journal_mode = DELETE` 并发出 `UnreliableFilesystem` 警告，同时在 `doctor` 中提示用户改用本地目录。

### 2.4 配置目录

配置目录解析优先级固定为：**CLI 全局 `--config-dir` > 环境变量 `ALIASMGR_CONFIG_DIR` > 平台默认值**。平台默认值为 Linux `${XDG_CONFIG_HOME:-$HOME/.config}/alias-manager/`、Windows `%LOCALAPPDATA%\AliasManager\`。所有测试通过 `ALIASMGR_CONFIG_DIR` 指向临时目录，禁止测试写入真实用户目录。

目录内包含：

```text
config.toml
aliases.db
generated/bash.sh
generated/zsh.sh
generated/powershell5.ps1
generated/powershell7.ps1
backups/
  rc/            用户 RC/Profile 备份
  generated/     生成文件备份
  db/            数据库迁移前备份
logs/
```

PowerShell 5.1 与 7 使用**独立生成文件**，因为编码策略（BOM）与部分语法差异不同，共用一个文件会导致其中一方乱码或不兼容。

**权限：** Linux 目录默认 `0700`，数据库和包含敏感配置的文件默认 `0600`；生成的 `.sh` 为 `0600`（仅需被用户自己 source）。Windows 使用当前用户 ACL，并在写入前拒绝明显可被其他用户写入的目录。

**保留与轮转策略（写入 `config.toml`，含默认值）：**

| 项目 | 默认 | 上限行为 |
|---|---|---|
| `backups.rc_keep` | 10 份/文件 | 超出按时间删除最旧 |
| `backups.generated_keep` | 10 份/Shell | 超出按时间删除最旧 |
| `backups.db_keep` | 5 份 | 超出按时间删除最旧 |
| `backups.max_total_bytes` | 64 MiB | 超出时从最旧开始清理，并记录警告 |
| `logs.max_file_bytes` | 8 MiB | 按天 + 大小轮转 |
| `logs.keep_files` | 7 | 超出删除最旧 |
| `retired_names.keep_revisions` | 20 | 超出该 revision 跨度的 tombstone 可清理，见 §5.1.1 |

清理只在成功完成一次同步后执行，且永不删除本次操作刚创建的备份。

---

## 3. 执行器和参数模型

### 3.1 结构化执行模型

例如：

```json
{
  "name": "cm",
  "target_type": "python_script",
  "executable": "python3",
  "fixed_args": ["/home/user/tools/copymv.py"],
  "pass_args": true,
  "working_directory": null,
  "shells": ["bash", "zsh"]
}
```

执行器先得到参数数组：

```text
["python3", "/home/user/tools/copymv.py", <用户参数...>]
```

Shell 适配器再将数组转换为目标 Shell 的安全字面量。禁止先拼接整行字符串，再交给 Shell 求值。

### 3.2 生成规则

- 默认生成函数包装器，以统一处理参数透传、未来扩展和当前会话清理。
- **“简单条件”定义（全部满足才可使用原生 alias/`Set-Alias`）：** 无固定参数、无 `{{args}}` 占位符、无参数透传、无工作目录、无环境变量、`target_type` 不是 `ChangeDirectory`、`executable` 是不含空格与特殊字符的简单命令名或绝对路径。任一条不满足即生成函数。
- 存在固定参数、用户参数透传、工作目录、环境变量或切换目录：必须生成函数。
- 强制覆盖已有 alias/function 时，必须把被覆盖对象的 Shell、名称、类型和原始定义（能安全读取时）写入 `overridden_definitions` 表，生成恢复提示或恢复脚本；无法解析时置 `recoverable = 0`；不能承诺重新加载一定恢复原定义。
- Bash/Zsh 外部命令调用使用 `command` 和 `"$@"`。
- PowerShell 外部命令调用使用 `&` 和 `@args`。
- `ChangeDirectory` 必须生成函数，并直接在当前 Shell 执行 `cd`；普通外部进程不能改变父 Shell 目录。
- `.py` 使用配置的 Python 解释器；`.jar` 使用 `java -jar`；`.ps1`、`.bat`、`.cmd` 使用适配器定义的调用方式（见 §3.5）。
- 工作目录处理：Bash/Zsh 使用 `( cd -- '<dir>' && command ... )` 子 shell，避免污染父 Shell 的当前目录；PowerShell 使用 `Push-Location` / `Pop-Location` 并置于 `try/finally` 中。`ChangeDirectory` 是唯一有意改变父 Shell 目录的类型。
- 环境变量处理：Bash/Zsh 使用 `env VAR=value command ...` 前缀形式，不使用 `export`；PowerShell 在函数内保存旧值、`try/finally` 中恢复。环境变量的值不得出现在日志或错误信息中。

预期生成示例：

```bash
unalias cm 2>/dev/null
cm() {
    command python3 '/home/user/tools/copymv.py' "$@"
}
```

```powershell
Remove-Item -LiteralPath Alias:\cm -Force -ErrorAction SilentlyContinue
Remove-Item -LiteralPath Function:\cm -Force -ErrorAction SilentlyContinue
function global:cm {
    & 'python.exe' 'C:\Tools\copymv.py' @args
}
```

`{{args}}` 中间插入示例（`fixed_args = ["build", "{{args}}", "--verbose"]`）：

```bash
unalias bd 2>/dev/null
bd() {
    command mytool 'build' "$@" '--verbose'
}
```

```powershell
function global:bd {
    & 'mytool.exe' 'build' @args '--verbose'
}
```

### 3.3 高级 Shell 模式（MVP 拒绝）

高级模式意在允许用户保存管道、重定向和逻辑连接。但如 §0.3.1 所述，它与“禁止字符串求值”原则根本对立，因此 **MVP 阶段 validation 直接拒绝**：

- `advanced_shell_mode = true` 或 `target_type = RawShellCommand` 一律返回 `AdvancedModeUnsupported`，不保存、不生成、不执行；
- 导入遇到此类记录时标记为 unsupported、计入安全报告并跳过，不写入数据库；
- 字段与枚举值保留，仅用于前向兼容和识别其他版本导出的数据；
- 不把普通模式自动升级为高级模式；
- 后续若启用，必须走独立的显式开关执行路径，并在文档中标注它不受原则 1 保护。

### 3.4 名称抢占处理（必须实现）

Shell 的名称解析优先级是 **alias 优先于 function**，这会导致两类实际失效：

- **Bash/Zsh：** 交互式 Shell 在解析期先做 alias 展开。若 `cm` 已是 alias（用户自定义、oh-my-zsh、发行版 `/etc/profile.d` 提供），`cm() { ... }` 会直接报 `syntax error near unexpected token '('`，整个生成文件后续内容全部不被加载。
- **PowerShell：** 查找顺序为 Alias → Function → Cmdlet → 外部程序。因此 `function global:ls` 无法覆盖内置 alias `ls`。受影响的常见名称包括 `ls`、`cp`、`mv`、`rm`、`cat`、`gc`、`gp`、`sl`、`si`、`gi` 等。这类失败**语法检查会通过**，只对文件内容做字符串断言的测试也会通过。

因此生成规则强制要求：

- Bash/Zsh：每个函数定义前输出独立一行 `unalias <name> 2>/dev/null`。该语句必须是**独立命令、位于函数定义之前的单独解析单元**（前面加空行分隔），不能与函数定义处于同一 parse unit，否则 alias 展开仍会先发生。
- PowerShell：函数定义前输出 `Remove-Item -LiteralPath Alias:\<name> -Force -ErrorAction SilentlyContinue` 与 `Remove-Item -LiteralPath Function:\<name> -Force -ErrorAction SilentlyContinue`。PS 5.1 无 `Remove-Alias`，统一使用 `Remove-Item` 以保持版本一致。
- 使用 `Set-Alias` 的简单路径必须带 `-Scope Global -Force`，并同样先清理同名 function。
- **ReadOnly/Constant 别名：** 部分 PowerShell 内置 alias 带 `ReadOnly` 或 `Constant` 选项，`Remove-Item` 会失败。`Constant` 无法在会话内移除。冲突检测阶段必须识别这类名称并返回 `NameReserved`，在创建时就阻止而不是等到运行时静默失败。已知保留名称清单需内置并可随版本更新。
- 抢占清理属于“强制覆盖”行为，必须走 §3.2 的 `overridden_definitions` 记录与用户提示流程。

### 3.5 参数透传的平台限制与转义规格

`@args` / `"$@"` 只保证 Alias Manager 自身不破坏参数数组，**不能消除宿主 Shell 的固有限制**。必须明确并测试以下三类：

**PowerShell → 原生 exe 的参数传递**

PowerShell 调用外部程序时会把参数重新拼接为命令行字符串，行为随版本变化：

| 版本 | 行为 | 影响 |
|---|---|---|
| PS 5.1 | 旧式拼接 | 含嵌入 `"`、空字符串 `""`、`--opt="a b"`、结尾反斜杠的参数会被破坏 |
| PS 7.0–7.2 | 旧式拼接（同上） | 同上 |
| PS 7.3+ | 默认 `PSNativeCommandArgumentPassing = Standard` | 大部分场景修复，但与旧脚本行为不一致 |

要求：适配器按目标版本选择转义策略；对 PS 5.1/7.2 已知无法安全传递的参数形态，在 `doctor` 与文档 `docs/limitations.md` 中明确列为已知限制，**不得声称完全安全**；不得为了绕过该限制而改用字符串求值。

**`.bat` / `.cmd` 调用**

`cmd.exe /c` 会对参数进行第二轮解析，转义规则与 PowerShell、与 exe 都不同（`%` 变量展开、`&`/`|`/`^` 需转义、引号规则不同）。因此：

- `TargetType::Batch` 必须有独立的执行器与独立的转义器，不复用 native exe 的转义逻辑；
- 参数中出现 `%`、`!`（延迟展开）、`&`、`|`、`^`、`<`、`>` 时必须转义或拒绝，并给出明确错误；
- 该转义器需要单独的单元测试与 Windows 集成测试。

**参数矩阵（所有 Shell 必测）**

空格、CJK/Unicode、空字符串 `""`、含 `"`、含 `'`、含反引号、含 `$`、含 `%VAR%`、结尾反斜杠 `C:\dir\`、通配符 `*` 与 `?`、含换行、含分号、超长命令行（Windows 8191 字符上限，需给出明确错误而非截断）。

---

## 4. Shell 适配器规格

### 4.1 检测顺序

Linux 检测顺序固定为：显式 `--shell` > 当前父进程链 > `$SHELL` > `/etc/passwd` 登录 Shell > 已安装 Shell 扫描 > 手动选择。GUI 不把桌面启动进程误认为当前交互 Shell，而是展示默认 Shell 和已安装 Shell，让用户选择。

### 4.2 配置文件

| Shell | 默认配置 |
|---|---|
| Bash | `~/.bashrc` |
| Zsh | `~/.zshrc` |
| PowerShell 5.1 | `$PROFILE.CurrentUserAllHosts` |
| PowerShell 7 | `$PROFILE.CurrentUserAllHosts` |

Bash 登录文件只在检测到没有加载 `.bashrc` 时提示用户，不擅自修改 `.bash_profile`、`.bash_login` 或 `.profile`。

**PowerShell Profile 路径必须实测获取，不允许拼接。** `$PROFILE.CurrentUserAllHosts` 默认指向 `<Documents>\WindowsPowerShell\profile.ps1`（PS 5.1）或 `<Documents>\PowerShell\profile.ps1`（PS 7），而 `Documents` 常被 OneDrive 或组策略重定向到 `%OneDrive%\Documents` 或网络位置。获取方式：

1. 首选执行 `powershell.exe -NoProfile -NonInteractive -Command "$PROFILE.CurrentUserAllHosts"` / `pwsh -NoProfile -NonInteractive -Command "$PROFILE.CurrentUserAllHosts"`，逐版本分别获取；
2. 该调用失败时回退到 `SHGetKnownFolderPath(FOLDERID_Documents)` 拼接，并记录为“回退路径”；
3. 两者都失败返回 `ConfigNotFound`，要求用户在 GUI/CLI 中显式指定 Profile 路径；
4. 解析出的路径落在网络位置（UNC 或已映射网络盘）时发出警告，提示 Profile 加载性能与锁可靠性问题。

**Profile 路径覆盖项：** `config.toml` 提供 `shells.powershell5.profile_path` 与 `shells.powershell7.profile_path`，用于（a）自动解析失败时由用户显式指定，(b) CI 与集成测试注入临时 Profile。这是必需能力而非可选项——`$PROFILE` 派生自 Documents 已知文件夹，无法仅靠环境变量重定向，没有该覆盖项就无法在不污染 runner 真实用户配置的前提下测试 PowerShell（见 §11.7）。同类覆盖项也提供给 Bash/Zsh 的 RC 路径。

**Bash 配置生效性诊断（`doctor` 必须覆盖）：**

- 非交互式 Shell 不读取 `.bashrc`，托管别名只在交互式 Shell 生效；
- 许多发行版的 `.bashrc` 开头带 `case $- in *i*) ;; *) return;; esac` 或 `[ -z "$PS1" ] && return` 提前返回，若加载块位于其后仍会执行（因为交互式才需要），但若被误插到 `return` 之前的非交互分支中则失效——`doctor` 需检查加载块相对这些守卫语句的位置；
- login shell 只读 `.bash_profile`/`.bash_login`/`.profile`，若这些文件都不 source `.bashrc`（常见于精简镜像与部分容器），别名在登录 Shell 中不生效——`doctor` 需检出并提示用户手工添加，程序不擅自修改；
- 检出 `.zshrc` 被 oh-my-zsh 安装脚本覆盖或备份为 `.zshrc.pre-oh-my-zsh` 时，提示加载块可能已丢失。

### 4.3 加载块

加载块中的生成文件路径由程序在渲染时写入**已解析的绝对路径**，并按目标 Shell 正确转义。不得在 Shell 内重新计算 `XDG_CONFIG_HOME` 或 `LOCALAPPDATA`，否则会与 `--config-dir` / `ALIASMGR_CONFIG_DIR` 覆盖机制不一致，测试环境与真实环境也会分叉。

Bash/Zsh 使用同一类标记：

```bash
# >>> Alias Manager >>>
# managed block, do not edit by hand
__aliasmgr_generated_file='/home/user/.config/alias-manager/generated/bash.sh'
[ -r "$__aliasmgr_generated_file" ] && . "$__aliasmgr_generated_file"
unset -v __aliasmgr_generated_file
# <<< Alias Manager <<<
```

PowerShell 使用（路径按 PS 版本分别指向 `powershell5.ps1` 或 `powershell7.ps1`）：

```powershell
# >>> Alias Manager >>>
# managed block, do not edit by hand
$__AliasMgrGeneratedFile = 'C:\Users\user\AppData\Local\AliasManager\generated\powershell7.ps1'
if (Test-Path -LiteralPath $__AliasMgrGeneratedFile) {
    . $__AliasMgrGeneratedFile
}
Remove-Variable __AliasMgrGeneratedFile -Scope Local -ErrorAction SilentlyContinue
# <<< Alias Manager <<<
```

变量名使用 `__aliasmgr_` / `__AliasMgr` 前缀，降低与用户变量冲突的概率；Bash 使用 `unset -v` 明确只清理变量，不影响同名函数。

**插入位置：** 加载块**只追加到配置文件末尾**。若插入在前部，oh-my-zsh、starship、conda、nvm、fzf 等在其后加载的组件会重新定义同名 alias，导致托管别名被静默覆盖。若检测到已有加载块不在末尾，`doctor` 报告“加载块位置可能被后续配置覆盖”，并提供“移动到末尾”的显式操作，但不自动执行。

**与用户插件的名称竞争：** 若用户的插件管理器在加载块之后重新定义同名 alias，Alias Manager 不参与竞争、不自动调整用户配置顺序，只在 `doctor` 中报告冲突及其来源文件行号（见原则 19）。

加载块插入和删除必须基于成对标记，不得用关键字搜索删除任意用户行。重复安装时先检测已有完整标记块，保证幂等。

**RC 文件的特殊形态：**

- **RC 文件是符号链接**（chezmoi、stow、dotfiles 仓库场景，非常常见）：不能一律拒绝。策略为：解析 symlink 目标，若目标位于当前用户拥有且非全局可写的路径下，则视为可信并**就地修改目标文件**（保持 symlink 不被替换，避免 dotfiles 仓库被破坏）；若目标跨用户、位于全局可写目录或链路中出现不可信环节，则返回 `UnsafePath` 并要求用户显式确认。原子替换时必须替换**目标文件**而非 symlink 自身。
- **RC 文件只读**：返回 `PermissionDenied`，输出需要用户手工粘贴的加载块内容。
- **RC 文件不存在**：按目标 Shell 的默认权限创建（Linux `0644`）。

### 4.4 生成文件编码和语法检查

PowerShell 5.1 对无 BOM 的 UTF-8 文件兼容性有限。PowerShell 生成器必须按目标版本写入独立文件：`powershell5.ps1` 使用 UTF-8 BOM；`powershell7.ps1` 使用 UTF-8 无 BOM。编码策略只作用于生成的 `.ps1`，不改变用户原有 Profile 的编码。修改用户 Profile 时必须**保留原文件的编码与行尾风格**（读取时探测 BOM 与 CRLF/LF，写回时保持一致）。

### 4.5 语法检查

- Bash：`bash -n generated/bash.sh`
- Zsh：`zsh -n generated/zsh.sh`
- PowerShell：调用 Parser API，不执行脚本：

```powershell
$tokens = $null
$errors = $null
[System.Management.Automation.Language.Parser]::ParseFile(
    $Path,
    [ref]$tokens,
    [ref]$errors
) | Out-Null
if ($errors.Count -gt 0) { exit 1 }
```

**ExecutionPolicy 与语言模式诊断（必须实现，不自动修改）：**

- Windows 客户端默认 `ExecutionPolicy` 为 `Restricted`，此时 **Profile 根本不会被执行**，所有 PowerShell 别名完全无效。`doctor` 必须检出并输出需要**用户自行执行**的指引：`Set-ExecutionPolicy -Scope CurrentUser RemoteSigned`。程序不代为执行、不使用 `-ExecutionPolicy Bypass` 绕过——后者只影响被启动的子进程，对用户交互 Shell 的 Profile 加载毫无帮助。
- `AllSigned` 策略下 dot-source 未签名的生成脚本会被拒绝。MVP 不支持脚本签名，因此该情形应报告为 `ExecutionPolicyBlocked` 并明确标注“当前不支持”，不做重试。
- 策略被 Group Policy（`MachinePolicy` / `UserPolicy` 作用域）锁定时，明确说明用户级无法覆盖，需联系管理员。
- ConstrainedLanguage 模式 / AppLocker 限制下函数定义可能受限，检出后同样报告为不支持。
- `doctor` 需分别报告 PS 5.1 与 PS 7 的策略，二者独立。

PowerShell 执行策略只做诊断，不自动降低策略，不绕过 Group Policy 或签名要求。

---

## 5. 同步、备份、回滚和并发

### 5.1 同步事务流程

```text
请求
  -> 校验别名和目标
  -> 获取全局文件锁
  -> 读取数据库当前 revision（revision_from）
  -> 创建用户配置和生成文件备份（记入 backups_json）
  -> 开启 SQLite 事务
  -> 更新 aliases、retired_names、revision（revision_to）与 operation_journal
  -> 从数据库快照逐 Shell 生成临时脚本
  -> 逐 Shell 执行语法检查
  -> 逐 Shell 原子替换生成文件，成功即更新该 Shell 的 shell_state
  -> 更新并提交 journal/数据库事务
  -> 释放锁并返回 per-shell 结果
```

所有生成操作必须从数据库快照开始；不得以磁盘上的生成脚本作为下一次写入的输入。应用启动时先扫描未完成的 journal：依据 `revision_from` / `revision_to` 判断方向——若数据库已提交到 `revision_to`，则前滚重建缺失/过期生成文件；若数据库事务未提交，则按 `backups_json` 回滚文件并清理临时文件。同步成功后生成文件头部保存 `revision` 和该文件的 `file_checksum`。

**多 Shell 部分成功语义（必须明确）：** 一次同步涉及多个 Shell 时，数据库 revision 统一递增，但各 Shell 的生成文件可能停留在不同 revision（例如 zsh 成功、PowerShell 语法检查失败）。因此：

- 每个 Shell 的实际状态记录在 `shell_state(applied_revision, file_checksum, status, last_error)`；
- `status` 取值：`ok`（`applied_revision == aliases.revision`）、`stale`（落后）、`failed`、`loader_missing`、`unknown`；
- `SyncReceipt` 返回 per-shell 结果，CLI/GUI 必须逐 Shell 展示成功与失败，不允许只输出一个整体“成功”；
- `doctor` 依据 `applied_revision` 与当前 revision 的差值报告“派生文件过期”，并提供 `aliasmgr sync --shell <shell>` 修复建议；
- 单个 Shell 失败不回滚其他 Shell 已成功的替换，也不回滚数据库；数据库始终是事实来源，落后的 Shell 靠重新同步收敛。

### 5.1.1 当前会话残留处理

删除或禁用别名只会修改数据库和派生配置，不能直接删除父 Shell 已加载的函数或 alias。

**为什么只靠“当前托管名称”不够：** 生成文件在每次同步时被整体替换。删除 `cm` 后，新生成文件的托管名称清单里不再包含 `cm`，因此 reload 永远清理不掉会话中残留的 `cm`。必须引入 **tombstone（退役名称）**。

**`retired_names` 表的写入时机：** 别名被删除、改名（旧名入表）或禁用（针对被移出的 Shell）时，将 `(name, shell, definition_kind, retired_at_revision)` 写入 `retired_names`。清理策略见 §2.4 的 `retired_names.keep_revisions`（默认保留 20 个 revision 跨度），超期条目在成功同步后删除。

**生成文件头部的清单 = 当前托管名称 ∪ 未过期的 tombstone：**

```text
# Alias Manager
# revision: 42
# file_checksum: 9f2c...
# managed: cm gs ll
# retired: oldcm tmpbuild
```

**清理执行方式：** 生成文件在定义任何别名**之前**，先按清单逐名清理，再写入当前 revision 的定义。清理必须按定义类型区分：

- Bash/Zsh：alias 用 `unalias <name> 2>/dev/null`，function 用 `unset -f <name> 2>/dev/null`；
- PowerShell：`Remove-Item -LiteralPath Alias:\<name> -Force -EA SilentlyContinue` 与 `Remove-Item -LiteralPath Function:\<name> -Force -EA SilentlyContinue`。

**误删用户定义的防护（重要）：** 如果我们曾强制覆盖过用户的同名 alias，用户之后又在自己的配置里重建了同名定义，无条件清理会误删用户的定义。因此清理只作用于“可确认由 Alias Manager 注册”的定义：

- 对函数：清理前用 `declare -f <name>`（Bash/Zsh）或 `(Get-Item Function:\<name>).Definition`（PowerShell）取出当前定义体，与本 revision/上一 revision 的生成内容指纹比对，一致才清理；
- 对 alias：同样比对 `alias <name>` 的值；
- 不一致时**跳过清理**，并把该名称记入本次 reload 的“跳过清单”，由 `doctor` 报告“会话中存在非托管的同名定义”；
- 指纹比对需要在生成文件中内联一份紧凑指纹表，不依赖 `aliasmgr` 可执行文件（满足原则 11）。

CLI/GUI 删除、禁用或强制覆盖后必须显示：当前会话可能仍有旧定义，需要执行对应 reload 命令；如果覆盖了用户原有 alias/function，从 `overridden_definitions` 提供恢复内容或明确标注不可自动恢复。不能声称子进程能够自动修改其父 Shell。

任何步骤失败都必须：

1. 不替换未通过检查的正式文件；
2. 恢复已替换的生成文件和数据库备份；
3. 将 journal 标记为失败或待恢复；
4. 向用户报告失败阶段和恢复结果。

### 5.2 原子写入

每个生成目标先写入同目录临时文件，例如 `bash.sh.tmp.<uuid>`；关闭并刷新文件后执行语法检查；检查通过后使用同一文件系统内的原子重命名替换正式文件。不得跨文件系统移动临时文件。

### 5.3 手工修改检测

生成脚本头部包含 `file_checksum`。同步前比较 `shell_state.file_checksum` 与磁盘文件实际内容的 checksum（计算时排除头部 `file_checksum` 行本身）：

- 未变更：正常重生成；
- 已手工修改：先创建备份，提示用户覆盖、取消或导出自定义片段；
- 不得静默丢弃手工修改。

### 5.4 文件安全

写入前检查父目录权限可接受、文件所有者正确。符号链接不一律拒绝，按 §4.3 的 symlink 策略处理（dotfiles 管理工具是常见合法场景）。使用锁避免 GUI 和 CLI 同时写入；锁超时（默认 10 秒，可配置）返回 `LockTimeout`，不删除其他进程的锁文件、不覆盖其他进程的修改。锁文件中记录持有者 PID 与启动时间，检测到持有进程已不存在时才允许接管，并记录日志。

**目标程序路径安全告警：** `executable` 或脚本路径位于全局可写目录时（Linux 的 `/tmp`、`/var/tmp`、任何 `other-writable` 目录；Windows 的 `C:\ProgramData` 下可写子目录、`C:\Windows\Temp`、当前用户以外可写的路径），必须发出 `UnsafeTargetLocation` 警告并要求用户确认——这类路径下的目标文件可被其他本地用户替换，等于把任意代码执行接口挂到用户的每个新 Shell 上。CLI 默认拒绝并要求 `--allow-unsafe-target`，GUI 显示醒目警告。

---

## 6. CLI 规格

### 6.1 命令树

全局参数（对所有子命令有效）：

```text
--config-dir <path>     覆盖配置目录，优先于 ALIASMGR_CONFIG_DIR
--format table|json     输出格式，默认 table
--no-color              禁用彩色输出（非 TTY 时自动生效）
--verbose / --quiet     日志级别
```

子命令：

```text
aliasmgr add <name> --exec <program> [--arg <value>]... [--shell <shells>]
                    [--cwd <dir>] [--env KEY=VALUE]... [--tag <tag>]...
                    [--no-pass-args] [--allow-missing] [--allow-unsafe-target]
aliasmgr remove <name> [--yes]
aliasmgr update <name> [fields...]
aliasmgr get <name> [--json]
aliasmgr find <query> [--fuzzy] [--field <field>] [--limit <n>]
aliasmgr list [--sort <field>] [--desc] [--shell <shell>] [--tag <tag>] [--limit <n>]
aliasmgr enable <name>
aliasmgr disable <name>
aliasmgr sync [--shell <shell>] [--dry-run]
aliasmgr reload [--shell <shell>] [--print]
aliasmgr doctor [--shell <shell>]
aliasmgr shell detect
aliasmgr shell install <shell>
aliasmgr shell uninstall <shell>
aliasmgr import <file> [--conflict skip|overwrite|rename|ask]
aliasmgr export <file>
aliasmgr uninstall [--purge-aliases]
```

### 6.2 CLI 行为要求

- `--shell auto` 使用检测顺序；CI 和脚本场景可用显式 Shell；
- 交互删除默认询问，`--yes` 禁止提示；非 TTY 环境下没有 `--yes` 的交互命令直接返回 `NonInteractive` 错误，不阻塞等待输入；
- `--format json` 输出稳定字段名，错误写 stderr；
- `--dry-run` 打印将要生成的脚本内容与将要修改的文件，不做任何写入；
- 不存在的可执行文件默认阻止同步，但允许 `--allow-missing` 保存为失效别名；
- 每种失败返回稳定非零错误码，见 §6.3；
- 同步成功后逐 Shell 提示当前已打开 Shell 需要执行的 reload 命令。

**`aliasmgr reload` 的定位：** CLI 是子进程，无法改变父 Shell 状态，这是硬约束。`reload --print` 输出可被父 Shell 求值的单行命令（如 `. '/home/user/.config/alias-manager/generated/bash.sh'`），供用户执行 `eval "$(aliasmgr reload --print)"`。这不违反原则 1：求值的是我们自己生成并已通过语法检查的文件路径，且求值发生在用户显式发起的命令中，不在生成的别名执行路径上。文档需给出推荐的 shell 函数包装示例，避免反复要求用户手工 `source`。

### 6.3 退出码表

自动化脚本依赖稳定退出码，映射必须固定并写入 `docs/exit-codes.md`：

| 码 | 含义 | 对应错误 |
|---|---|---|
| 0 | 成功 | — |
| 1 | 未分类错误 | 兜底 |
| 2 | 用法错误 | clap 解析失败、参数组合非法 |
| 3 | 名称/定义冲突 | `AliasConflict`、`NameReserved` |
| 4 | 未找到 | 别名不存在、`ConfigNotFound` |
| 5 | 校验失败 | `InvalidAliasName`、`InvalidArgTemplate`、`AdvancedModeUnsupported` |
| 6 | 目标缺失或不安全 | `TargetMissing`、`UnsafePath`、`UnsafeTargetLocation` |
| 7 | 权限问题 | `PermissionDenied` |
| 8 | 锁超时 | `LockTimeout` |
| 9 | 语法检查失败 | `SyntaxCheckFailed` |
| 10 | 同步部分失败 | 至少一个 Shell 失败，其余成功 |
| 11 | 回滚失败（需人工介入） | `RollbackFailed` |
| 12 | 环境不支持 | `ShellNotInstalled`、`ExecutionPolicyBlocked`、`SchemaTooNew` |
| 13 | 存储错误 | `DatabaseError`、`UnreliableFilesystem`（致命时） |
| 14 | 需要交互但无 TTY | `NonInteractive` |

退出码一经发布不得改变含义；新增错误只能追加新码或归入现有语义。

---

## 7. GUI 规格

GUI 使用 Tauri 2，所有业务操作调用 Rust 核心命令，不复制 CLI 业务逻辑。**GUI 属于 MVP 基础需求，Linux 与 Windows 均需交付。**

### 7.1 GUI 页面（MVP 范围）

- 主列表：状态、别名、目标、类型、Shell、参数透传、更新时间、冲突状态、per-shell 同步状态（`ok`/`stale`/`failed`）；
- 搜索：名称、目标、描述、标签、Shell、启用状态和模糊匹配；
- 新增/编辑向导：名称、目标类型、结构化参数（含 `{{args}}` 占位符可视化）、工作目录、环境变量、标签、Shell、预览、测试确认；
- 详情页：文件存在性、生成代码、配置路径、最近同步结果、诊断信息、被覆盖的原定义与恢复入口；
- 设置：备份轮转、日志轮转、默认 Shell、配置目录、相对路径开关、卸载保留/清理策略。

### 7.2 测试运行

测试按钮必须显示最终参数和可能的副作用，要求用户主动确认。普通模式测试通过 Rust `Command` 数组执行，不经过 Shell；高级模式测试在 MVP 中不执行。

---

## 8. 搜索、排序、冲突和导入导出

### 8.1 搜索

搜索字段包括名称、目标、脚本路径、描述、标签、Shell 和目标类型。评分权重必须以常量形式集中定义并写入 `docs/architecture.md`，保证结果可复现：

| 匹配类型 | 权重 |
|---|---|
| 名称完全匹配 | 1000 |
| 名称前缀匹配 | 800 |
| 名称连续子串 | 600 |
| 名称非连续字符（fuzzy） | 400 |
| 标签完全匹配 | 350 |
| 描述/目标连续子串 | 200 |
| 编辑距离 ≤ 2 | 100 |

相同评分按名称升序、再按 `updated_at` 降序稳定排序。`--limit` 默认 50（`--limit 0` 表示不限制），GUI 默认分页 100 条。排序字段：`name`、`updated_at`、`created_at`、`target_type`、`enabled`。

### 8.2 名称校验

跨平台推荐正则：`^[A-Za-z_][A-Za-z0-9_-]*$`。拒绝空格、`=`、`;`、`$`、引号和 Shell 控制字符。长度上限 64。冲突检查包括：

- 已有托管别名（含 §2.3 的大小写折叠冲突检查）；
- 目标 Shell 中已存在的原生 alias、函数、内置命令；
- PATH 中的可执行文件（Windows 需按 `PATHEXT` 逐后缀查找，不只查 `.exe`）；
- PowerShell Cmdlet 与内置 alias；
- **保留名称清单：** PowerShell 中带 `ReadOnly`/`Constant` 选项的内置 alias 无法被安全移除，命中返回 `NameReserved` 直接拒绝创建（见 §3.4）。

默认不静默覆盖。可覆盖的冲突需用户显式确认并记录到 `overridden_definitions`。

### 8.3 导入导出

导出文件必须包含顶层 `format_version` 字段（整数，独立于数据库 `user_version`）与 `exported_at`、`exported_by_version`。导入时：`format_version` 高于程序支持范围则拒绝并提示升级；低于则按迁移规则升级后导入。缺失该字段视为不受支持的旧格式并拒绝。

导出 JSON/TOML 时包含结构化别名字段，不导出密码、Token、API Key 或敏感环境变量。`advanced_shell_mode`/`RawShellCommand` 记录在导入时标记为 unsupported、计入安全报告并跳过（见 §3.3）。导入配置等同于导入可能执行的代码，导入流程必须分为：读取而不执行、解析和 schema 校验、目标路径/解释器/高级模式安全扫描、展示逐条预览和风险报告、用户明确确认、写入数据库、语法检查、同步。任何预览或验证失败都不得自动同步。

安全报告至少标记高级 Shell 命令、管道、重定向、命令替换、未知路径、网络路径、全局可写目录中的目标（`UnsafeTargetLocation`）、缺失程序、绝对路径变化、潜在危险命令、敏感环境变量、保留名称和强制覆盖现有定义。冲突策略 `overwrite` 也必须再次确认，且提示当前会话中的原定义可能无法自动恢复。现有配置扫描只解析简单 `alias name='value'`，复杂函数和动态逻辑必须标记为人工确认，不执行配置文件。

### 8.4 路径策略

GUI 文件选择器返回的可执行文件、脚本和 JAR 默认规范化为绝对路径，并在数据库中保存路径类型和来源。相对路径只有在用户显式勾选“允许相对路径”后才可保存；运行时相对路径相对于明确记录的工作目录解析，不能依赖启动 GUI 的当前目录。导入相对路径时必须要求用户选择基准目录或拒绝导入。

---

## 9. 安全、日志和错误处理

### 9.1 安全要求

- 默认不提权；
- 不调用 `eval`/`Invoke-Expression`；
- 所有路径使用字面量和参数数组；
- 避免将环境变量中的敏感值写入日志、导出文件和错误消息；
- 不自动降低 PowerShell 执行策略；
- 系统级 `/etc/profile.d` 和全局 PowerShell 配置只能作为明确授权的后续功能。

### 9.1.1 非功能性指标（NFR）

| 指标 | 目标 |
|---|---|
| 支持别名规模 | ≥ 1000 条 |
| 生成文件加载耗时 | 500 条别名时 Bash/Zsh source < 30ms；PowerShell dot-source < 150ms |
| CLI 冷启动到输出 | `aliasmgr list`（500 条）< 150ms |
| 单次同步（全 Shell，500 条） | < 500ms（不含 Shell 语法检查进程启动） |
| GUI 首屏可交互 | < 1.5s |

PowerShell 的 Profile 加载时间是用户感知最强的部分（直接体现为终端启动变慢），因此生成代码必须紧凑：避免逐条别名的重复样板、把清理清单合并为循环、避免在生成文件中做路径探测或外部进程调用。M2 结束时必须用 500/1000 条别名做一次加载耗时基准并记录在 `docs/testing.md`；超标则考虑按需加载或代码压缩策略。

### 9.2 日志字段

记录操作类型、别名 ID、Shell、配置路径、生成结果、语法检查结果、回滚结果和错误码；不记录密码、Token、敏感参数或脚本输出隐私内容。日志按用户目录保存并支持轮转。

### 9.3 错误分类

核心错误至少包括：`InvalidAliasName`、`InvalidArgTemplate`、`AliasConflict`、`NameReserved`、`AdvancedModeUnsupported`、`ShellNotInstalled`、`ConfigNotFound`、`PermissionDenied`、`UnsafePath`、`UnsafeTargetLocation`、`SyntaxCheckFailed`、`LockTimeout`、`DatabaseError`、`SchemaTooNew`、`UnreliableFilesystem`、`RollbackFailed`、`ExecutionPolicyBlocked`、`NonInteractive` 和 `TargetMissing`。CLI 按 §6.3 的表格映射为稳定退出码，GUI 显示可操作的解决建议。

---

## 10. 测试规格

### 10.1 单元测试

覆盖：名称校验、长度上限、保留名称、大小写折叠冲突、`{{args}}` 占位符（缺省/中间/重复/转义/与 `pass_args=false` 组合）、参数数组边界、中文路径、空格路径、反斜杠路径、Shell 转义、`.bat`/`.cmd` 独立转义器、函数/alias 选择的“简单条件”边界、名称抢占清理代码生成、tombstone 清单生成、定义指纹比对跳过逻辑、PowerShell 生成、加载块幂等、加载块追加位置、标记块删除、编码与行尾保持、搜索评分权重、排序稳定性、迁移执行器、`SchemaTooNew` 拒绝、事务回滚、per-shell `shell_state` 状态迁移、`record_checksum` 与 `file_checksum` 区分、退出码映射和冲突检测。

### 10.2 集成矩阵

| 平台 | Shell | 重点 |
|---|---|---|
| Ubuntu | Bash | 增删改查、`"$@"` 透传、`.bashrc` 加载块 |
| Ubuntu | Zsh | 函数生成、语法检查、`.zshrc` 加载块 |
| Ubuntu + oh-my-zsh | Zsh | 已存在同名 alias 的抢占、加载块位置、插件后置覆盖检测 |
| Fedora | Bash | XDG 路径和权限 |
| Linux（dotfiles symlink） | Bash | RC 为 symlink 时就地修改目标文件、symlink 不被替换 |
| Windows 10 | PowerShell 5.1 | Profile、EXE、BAT、执行策略诊断、UTF-8 BOM |
| Windows 11 | PowerShell 7 | 函数、Python、PS1、多个宿主、无 BOM |
| Windows（Restricted 策略） | PS 5.1 | `doctor` 检出 Profile 不执行并给出正确指引 |
| Windows（OneDrive 重定向 Documents） | PS 5.1/7 | Profile 路径实测解析而非拼接 |
| Windows Terminal | PS 5.1/7 | 版本隔离和 Profile 定位 |
| Linux + Windows | — | GUI 端到端（Playwright）：列表、向导、同步、诊断、卸载设置 |
| WSL | Bash/Zsh | 路径与父进程检测，作为后续阶段 |

### 10.3 关键验收用例

1. 创建 `cm`，目标为 Python 脚本，执行 `cm cp "file name.txt"`，验证目标程序收到 `cp` 和 `file name.txt` 两个参数。
2. **名称抢占：** 在 `.bashrc` 中预先定义 `alias cm='echo x'`，同步后加载生成文件不产生语法错误，且 `cm` 指向托管定义。
3. **PowerShell 内置 alias 抢占：** 创建名为 `ls` 的托管别名，在 PS 5.1 与 PS 7 中验证 `Get-Command ls` 返回托管 Function 而非内置 Alias。
4. **保留名称拒绝：** 尝试创建与 `ReadOnly`/`Constant` 内置 alias 同名的别名，验证在创建阶段返回 `NameReserved`（退出码 3），而不是同步后静默失效。
5. **`{{args}}` 中间插入：** `fixed_args = ["build", "{{args}}", "--verbose"]`，执行 `bd x y` 验证目标收到 `build x y --verbose`。
6. **tombstone 清理：** 同一会话内创建 `tmpx` → reload → 删除 `tmpx` → reload，验证会话中 `tmpx` 已不存在（该用例是 §5.1.1 机制的核心回归，必须在真实 Shell 中执行）。
7. **指纹保护：** 覆盖用户的 `alias foo` 后，用户在会话中手工重建 `alias foo='own'`，再次 reload 时验证用户定义未被清理，并在 `doctor` 中被报告。
8. 删除 `cm`，验证数据库、生成文件不含 `cm`，加载块仍存在且用户其他配置不变。
9. 手工修改生成文件后同步，验证备份生成并要求用户选择，不静默覆盖。
10. 让语法检查失败，验证正式脚本和数据库恢复到上一个有效版本。
11. **多 Shell 部分失败：** 令 PowerShell 语法检查失败而 Bash 成功，验证 `shell_state` 分别为 `failed` 与 `ok`、退出码为 10、`doctor` 报告 PowerShell 过期。
12. **`SchemaTooNew`：** 手工把 `user_version` 改为更大值，验证程序拒绝打开并返回退出码 12，不写入任何数据。
13. 选择卸载保留，移除应用后生成函数仍能直接调用 Python/EXE，不依赖 `aliasmgr`。
14. 选择卸载删除，验证只删除 Alias Manager 标记块、生成文件和用户指定备份，其他 Profile 内容保留。
15. 同时运行 GUI/CLI 写操作，验证锁阻止竞态覆盖并返回 `LockTimeout`（退出码 8）。
16. **参数矩阵：** 按 §3.5 的完整矩阵对每个 Shell 执行 `argument-dumper`，逐项断言 argv；PS 5.1/7.2 上已知失败的形态必须被显式标记为 expected-limitation 而非跳过。

---

# 第二部分：可执行路线图

## 11. 远程验证与 CI 驱动开发流程

### 11.1 基本约定

**本项目采用 CI 驱动开发（remote-first verification）。** 开发者本地不安装 Rust 工具链、Zsh、WebKitGTK、Visual Studio Build Tools、PowerShell 7 等工具，所有编译、测试与打包在 GitHub Actions 的 Linux/Windows runner 上执行。AI 通过本地已安装的 `gh` 读取运行结果并迭代。

标准迭代闭环：

```text
本地 AI 编码
  -> 提交到 feature 分支
  -> 推送 GitHub
  -> GitHub Actions 在 Linux/Windows runner 测试与构建
  -> AI 通过 gh 读取结论与失败日志
  -> 定位失败原因并修改代码
  -> 再次提交触发验证
```

### 11.1.1 本地环境实测结论（决定命令在哪里执行）

本地环境已实测确认：

| 工具 | 状态 | 说明 |
|---|---|---|
| `git` | 已安装（2.53.0.windows.1） | 提交与推送 |
| `gh` | 已安装（2.97.0），**不在 PATH** | 位于 `D:\Program Files\GitHub CLI\gh.exe`，**必须使用绝对路径调用** |
| `node` / `npm` | 已安装（v24.16.0 / 11.13.0） | 可做前端本地自检 |
| `python` | 已安装（3.12.3） | 可本地校验测试夹具脚本语法 |
| `cargo` / `rustc` / `rustup` | **未安装** | **本地无法执行任何 `cargo` 命令** |

**结论：本地不存在 Rust 工具链，因此计划中所有 `cargo` 命令（含 `cargo fmt`、`cargo check`、`cargo clippy`、`cargo test`、`cargo tauri build`/`dev`）一律不在本地执行，只在 GitHub Actions 上执行。** 本项目不要求用户安装 Rust——这正是采用 CI 驱动开发的原因。

`gh` 调用必须写绝对路径（裸 `gh` 在 PATH 中不存在，会直接失败）：

```bash
"D:/Program Files/GitHub CLI/gh.exe" run list --branch <branch> --limit 5
```

`gh` 当前**未登录**（`You are not logged into any GitHub hosts`），需用户先执行认证，见 §11.2。

### 11.1.2 命令执行位置约定（消除歧义）

计划中所有任务清单条目按以下规则判定执行位置：

| 标记 | 含义 |
|---|---|
| `【CI】` | **只在 GitHub Actions 上执行。** AI 不得尝试在本地运行；本地运行必然因缺少工具链而失败，这类失败不是代码缺陷。 |
| `【本地】` | 在本地执行（仅限 `git`、`gh` 绝对路径、`npm`、`python`、文件读写）。 |
| 无标记 | 编码/设计类步骤（写代码、写 SQL、写文档），不涉及命令执行。 |

判定规则（标记缺失时以此为准）：

1. 任何 `cargo *` 命令 → **`【CI】`**，无例外；
2. 任何需要 Bash、Zsh、PowerShell 5.1/7、WebKitGTK、MSVC 的验证 → **`【CI】`**；
3. 任何打包（AppImage/deb/MSI/EXE）→ **`【CI】`**；
4. `git` 提交推送、`gh` 查询、`npm` 前端自检、`python` 夹具语法检查 → `【本地】`；
5. GUI 的可启动性与交互确认 → 既不在本地也不在 CI，由用户下载 artifact 后人工确认（§11.9）。

**由此产生的完成判定规则（强制）：** 任何“测试通过”“已验证”“已修复”的结论必须以 **GitHub Actions 的运行结果**为证据，并在汇报时给出 run id 与结论。禁止基于“代码看起来对了”或本地未执行的命令宣称通过；本地缺少 `cargo` 意味着不存在“本地跑过了”这种可能性。任务清单中的 `【CI】执行 cargo test ...` 应理解为“该命令已由 CI 执行并通过”，而不是“AI 在本地执行过”。

### 11.2 授权与操作边界（必须遵守）

`gh` 涉及账户凭据与远程写操作，因此：

1. **`gh auth login` 及任何认证流程必须由用户本人执行**，AI 不得代为输入凭据、不得读取或打印 token、不得修改 `~/.config/gh/hosts.yml`。AI 只能在需要时提示用户执行 `! "D:/Program Files/GitHub CLI/gh.exe" auth login` 或 `... auth status`。
2. 首次需要创建远端仓库、修改仓库设置、启用 Actions、添加 Secrets 时，AI 必须先说明用途并取得用户明确同意，不擅自执行。
3. AI 只在 **feature 分支**上提交与推送；不直接推送 `main`，不使用 `--force`（含 `--force-with-lease`）除用户明确要求，不删除远端分支，不改写已推送的历史。
4. 合并到 `main` 由用户决定（PR 审阅或明确指示），AI 不自动合并、不自动创建 Release、不自动打 tag。
5. AI 不得在提交内容或日志中写入任何凭据；签名证书、token 一律通过 GitHub Secrets 由用户配置。

### 11.3 迭代规则（防止无效轮询与资源浪费）

- **一次迭代 = 一个语义完整的 commit。** 不允许为了触发 CI 而提交空 commit 或 `fix ci` 系列无意义提交。**本地没有 Rust 工具链，无法预先排除编译错误**，因此提交前必须靠仔细的静态审查（类型、借用、模块可见性、依赖项、feature 开关）压低往返次数——每次编译错误都要花一整轮 CI 才能发现。
- 推送后使用 `gh run watch --exit-status` 等待，而不是循环 `sleep` + `gh run list` 轮询。
- 读取失败信息优先使用 `gh run view <id> --log-failed`，**不要下载完整日志**（完整日志包含大量无关输出，既慢又浪费上下文）。
- 若同一根因连续失败 **3 次**仍未解决，停止迭代，向用户汇报已知信息与假设，请求决策；不要继续盲目试错。
- 每次修改必须基于日志中的具体错误定位，禁止“改点东西再推一次看看”。
- 若失败原因是 CI 环境本身（依赖包名变更、runner 镜像升级），修 workflow，并在 commit message 中写明。

常用命令（供实现与文档参考）。**所有命令中的 `gh` 均需替换为绝对路径 `"D:/Program Files/GitHub CLI/gh.exe"`**（§11.1.1）：

```bash
gh auth status                              # 由用户执行
gh run list --branch <branch> --limit 5
gh run watch <run-id> --exit-status
gh run view <run-id>                        # 各 job 结论概览
gh run view <run-id> --log-failed           # 只看失败步骤日志
gh run view --job <job-id> --log-failed
gh run rerun <run-id> --failed              # 仅在怀疑瞬时故障时使用
gh workflow run ci.yml --ref <branch>       # 手动触发
gh run download <run-id> -n <artifact>      # 需要产物时（GUI 人工确认取包）
```

### 11.4 Workflow 划分

拆成三个文件，不把所有任务塞进一个 workflow：

**`ci.yml`** — `push`、`pull_request`、`workflow_dispatch` 触发，提供快速反馈。

| Job | Runner | 内容 |
|---|---|---|
| `lint` | `ubuntu-latest` | `cargo fmt --check`、`cargo clippy --workspace --all-targets -- -D warnings` |
| `test-linux` | `ubuntu-latest` | `cargo test --workspace`、SQLite 迁移测试、Bash/Zsh 适配器测试 |
| `test-windows` | `windows-latest` | `cargo clippy`（平台分支代码）、`cargo test --workspace`、PowerShell 5.1 与 7 分别测试、Profile 定位、Parser 检查、UTF-8 BOM 校验、Windows CLI 测试 |
| `gui-build-linux` | `ubuntu-latest` | 前端单元测试 + 类型检查 + Tauri Linux 编译 |
| `gui-build-windows` | `windows-latest` | 前端单元测试 + Tauri Windows 编译 |

`cargo fmt --check` **只在 Linux 跑一次**（格式与平台无关，重复执行只是浪费时间）；`clippy` 两个平台都要跑，因为 `#[cfg(windows)]` / `#[cfg(unix)]` 分支下的代码只在对应平台被检查。

**`integration.yml`** — 慢速集成测试，`workflow_dispatch` + `main` 的 `push` + 针对核心路径的 `pull_request` 触发：

- 加载块插入/删除/幂等/位置；
- 参数透传完整矩阵（§3.5）；
- 同名 alias/function 抢占（§3.4，含 oh-my-zsh 环境与 PowerShell 内置 `ls`）；
- tombstone 会话清理与指纹保护（§5.1.1）；
- revision、`shell_state`、operation journal 与多 Shell 部分失败；
- 崩溃恢复（中断后启动恢复扫描）；
- 卸载保留/删除；
- 加载耗时基准（§9.1.1）。

**`release.yml`** — 仅 tag 推送或 `workflow_dispatch` 触发：

- 构建 Linux AppImage/deb 与 Windows MSI/EXE；
- 上传 artifacts；
- 可选创建 GitHub Release（需用户明确指示）。

开发早期不在每次 commit 上构建安装包——构建时间与日志量都会显著增加，干扰核心逻辑的迭代速度。

### 11.5 Runner 环境与依赖

**Linux（`ubuntu-latest`）需通过 `apt-get` 安装：**

```text
zsh
libwebkit2gtk-4.1-dev
libayatana-appindicator3-dev
librsvg2-dev
patchelf
build-essential curl wget file libssl-dev libgtk-3-dev
```

注意事项（必须在 M0 验证，容易踩坑）：

- `ubuntu-latest` 当前为 24.04，Tauri 2 对应 `libwebkit2gtk-4.1-dev`；若因兼容性回退到 `ubuntu-22.04`，则需改用 `libwebkit2gtk-4.0-dev`。**workflow 中必须固定 runner 版本（`ubuntu-24.04` / `ubuntu-22.04`）而不是使用 `ubuntu-latest`**，避免镜像升级导致依赖包名突然失效。
- `libappindicator3-dev` 在 24.04 上已被 `libayatana-appindicator3-dev` 取代，直接沿用旧包名会安装失败。
- **AppImage 的 glibc 向下兼容性取决于构建机**：在 24.04 上构建的 AppImage 无法在较旧发行版运行。`release.yml` 的 Linux 打包 job 应固定使用较旧的 `ubuntu-22.04`，`ci.yml` 的编译检查可用较新版本。

**Windows（`windows-latest`）已预装，无需安装：** Visual Studio Build Tools、MSVC、Windows SDK、Windows PowerShell 5.1、PowerShell 7（`pwsh`）、WebView2、Node.js。workflow 只需在日志开头打印两个 PowerShell 的版本号，便于日后排查 runner 镜像变更引起的行为差异。

**PowerShell 5.1 与 7 必须分开测试，不能用 7 代表两者。** 两者的 Profile 路径、默认编码、Parser 行为、原生命令参数传递（§3.5）都不同。在 GitHub Actions 中区分方式为 step 的 `shell` 键：

```yaml
- name: PowerShell 5.1 tests
  shell: powershell        # Windows PowerShell 5.1
  run: ...
- name: PowerShell 7 tests
  shell: pwsh              # PowerShell 7
  run: ...
```

**其他固定项：** 使用 `actions/setup-node`、`actions/setup-python`、`dtolnay/rust-toolchain` 并**固定版本号**，不使用浮动的 `latest`；使用 `Swatinem/rust-cache` 与 npm 缓存以压缩反馈时间。

### 11.6 CI 性能与成本控制

迭代频率高时这些直接决定反馈速度：

- 每个 workflow 设置 `concurrency: group: <workflow>-<ref>` 与 `cancel-in-progress: true`，新推送自动取消同分支的旧运行；
- `strategy.fail-fast: false`，让 Linux 与 Windows 的失败在同一次运行中全部暴露，减少往返次数；
- 每个 job 设置 `timeout-minutes`（lint/test 20，GUI build 40），避免卡死消耗额度；
- 用 `paths-ignore` 跳过纯文档改动；GUI build job 仅在 `crates/aliasmgr-gui/**` 或核心库变更时运行；
- Windows runner 计费倍率高于 Linux，慢速集成测试尽量放在 `integration.yml` 而非每次 push 的 `ci.yml`。

### 11.7 CI 的测试隔离要求（强制）

**CI 绝不允许修改 runner 的真实用户配置。** 每个测试必须在隔离环境中运行：

- Linux：临时 `HOME`、临时 `XDG_CONFIG_HOME`、临时 `.bashrc` / `.zshrc`；
- Windows：临时 `LOCALAPPDATA`、`APPDATA`、`USERPROFILE`，以及临时 PowerShell Profile；
- 所有测试统一通过 `ALIASMGR_CONFIG_DIR` 指向临时目录（§2.4）。

**由此产生一条对核心库的设计要求：** PowerShell 的 Profile 路径来自 `$PROFILE`，它派生于 Documents 已知文件夹，**无法仅靠设置环境变量重定向**。因此核心库必须提供显式的 Profile 路径注入点（例如 `config.toml` 中的 `shells.powershell5.profile_path` / `shells.powershell7.profile_path` 覆盖项，或等价的构造参数），供测试与高级用户使用；该覆盖项同时满足 §4.2 中“解析失败时由用户显式指定 Profile 路径”的要求。不允许为了可测性而在生产路径中放宽真实 Profile 的写入范围。

测试结束必须清理临时目录；**禁止把 runner 或本地用户的真实配置文件上传为 artifact**。

### 11.8 CI 日志与产物的敏感信息防护

- 禁止 `env`、`Get-ChildItem Env:`、`printenv` 等环境变量整体转储；
- 禁止在失败时直接打印完整 Shell 配置文件内容；确需诊断时只输出 Alias Manager 标记块范围内的行；
- Bash step 不启用 `set -x`；
- 上传的生成脚本快照必须经过脱敏（去除环境变量值、用户名路径片段），artifact 保留期设为 7 天；
- 任何密钥通过 GitHub Secrets 注入，并在必要时使用 `::add-mask::`；来自 fork 的 PR 拿不到 Secrets，因此签名相关步骤只在 tag 触发的 `release.yml` 中执行；
- 签名证书不得提交进仓库；未签名的构建产物可以先用于测试验证。

### 11.9 远程验证的能力边界（必须向用户如实说明）

GitHub Actions **能**替代本地的部分：编译、单元测试、Shell 集成测试、CLI 测试、Tauri 构建、安装包构建、跨平台矩阵。

**不能**替代的部分，任何“已验证”的结论都不得覆盖这些：

- GUI 的视觉呈现与交互体验；
- 当前用户真实 Windows 配置的兼容性（OneDrive 重定向、组策略锁定的 ExecutionPolicy、企业 AppLocker）；
- 本机已有 Profile 的安全性验证（备份是否真的可恢复用户的实际配置）；
- 真实终端行为与长时间运行表现（Windows Terminal / GNOME Terminal 的实际启动耗时体感）；
- 用户实际安装的插件管理器（oh-my-zsh 自定义配置、starship、conda）造成的名称竞争全貌。

因此发布前的“干净环境安装/卸载验收”（Task 22）与 GUI 交互确认仍需在真实机器上人工完成。GUI 的端到端交互测试（Playwright/tauri-driver）在 Linux 上需要 `xvfb` 与 `WebKitWebDriver`、在 Windows 上需要匹配的 Edge WebDriver，属于阶段三加入项，先以“前端单元测试 + Tauri 编译通过”作为 CI 门槛。

### 11.10 CI 能力的引入节奏

避免 GUI 构建问题干扰核心逻辑开发，分三步启用：

| 阶段 | 时机 | CI 内容 |
|---|---|---|
| 一 | M0–M3 | Rust core、SQLite 迁移、Bash、Zsh、PowerShell 5.1、PowerShell 7、CLI |
| 二 | M4–M5 | 追加 Tauri Linux/Windows 编译、前端单元测试、GUI 类型检查 |
| 三 | M6 | 追加 AppImage/deb、MSI/EXE 打包、卸载测试、升级测试、artifact 下载验证 |

`ci.yml` 的骨架在 Task 1（M0）就必须建立并跑通一次，否则后续每个任务都缺少验证手段；GUI 与打包相关 job 按上表逐步加入。

---

## 12. 里程碑和依赖

| 里程碑 | 结果 | 前置 |
|---|---|---|
| M0 | Workspace、规范、`ci.yml` 骨架在 Linux/Windows 双 runner 跑通、测试夹具可运行；Tauri 依赖与目标发行版范围确认 | 无 |
| M1 | 核心模型、迁移执行器、SQLite、校验、搜索完成 | M0 |
| M2 | Bash/Zsh/PowerShell 生成、名称抢占、tombstone 清理与同步完成；加载耗时基准 | M1 |
| M3 | Linux CLI 可用（含退出码表与 `reload`） | M2 |
| M4 | Windows CLI 与卸载流程可用 | M3 |
| M5 | Tauri GUI 在 Linux 与 Windows 可用（**MVP 交付物**） | M3 |
| M6 | 导入导出、发布质量与验收完成 | M4、M5 |
| M7 | 扩展 Shell（Fish/POSIX/WSL/CMD 等）与插件边界 | M6 |

**MVP 边界 = M0–M6。** M5 的 GUI 属于 MVP，不可裁剪；M7 为 MVP 之后的扩展。

每个里程碑结束时必须运行全量测试、更新文档、检查安全要求，并提交一个可独立回滚的 commit。

---

## 13. 阶段一：Workspace、核心模型和数据库

### Task 1：初始化 Rust workspace

**Files:**
- Create: `Cargo.toml`
- Create: `crates/aliasmgr-core/Cargo.toml`
- Create: `crates/aliasmgr-core/src/lib.rs`
- Create: `crates/aliasmgr-cli/Cargo.toml`
- Create: `crates/aliasmgr-cli/src/main.rs`
- Create: `crates/aliasmgr-tests/Cargo.toml`
- Create: `.gitignore`
- Create: `rust-toolchain.toml`

- [x] 写一个最小 `aliasmgr-core` 单元测试，验证 crate 可以被 workspace 构建。
- [x] 【CI】在 GitHub Actions 上执行 `cargo test --workspace`，预期输出 `test result: ok`。
- [x] 配置依赖版本和 workspace lint，保证 CLI 依赖 core 而非反向依赖。
- [x] 【CI】明确依赖 features：`rusqlite = { version = "*", features = ["bundled"] }`，并在 Windows runner 上验证无需系统 SQLite 即可构建。
- [x] 【CI】选定并验证文件锁库（`fs4` 优先），在 Linux 与 Windows runner 上各跑一个跨进程加锁的冒烟测试。
- [x] 确认 Tauri 2 的 Linux 系统依赖（`webkit2gtk-4.1` 等）与最低支持发行版，写入 `docs/architecture.md`。
- [x] 【CI】建立 `crates/aliasmgr-tests` 空集成测试 crate，在 CI 上验证 `cargo test -p aliasmgr-tests` 能运行（确认集成测试不放在 workspace 根 `tests/`，否则不会被编译）。
- [x] 提交 `chore: initialize Rust workspace`。

Task 1 CI enhancements adopted:
- Rust cache: implemented with `Swatinem/rust-cache` in fast and integration workflows.
- npm cache: deferred until GUI jobs enter CI; no frontend dependency install exists in the current workspace.
- Strict isolation: implemented with temporary HOME/config paths and CI assertions.
- Independent-process lock test: implemented in `aliasmgr-tests` and the integration matrix.
- Slow integration matrix: implemented in `integration.yml` for Linux and Windows lock/core coverage.

### Task 1.1：打通 GitHub Actions 验证闭环（M0 的实际门槛）

**Files:**
- Create: `.github/workflows/ci.yml`（骨架，仅 lint + 双平台 `cargo test`）
- Create: `docs/ci-workflow.md`

由于本项目所有跨平台验证都依赖 CI（§11），`ci.yml` 必须在写业务代码之前跑通，否则后续每个任务都没有验证手段。

- [x] 【本地】提示用户执行 `! "D:/Program Files/GitHub CLI/gh.exe" auth login`（当前状态为未登录）；**AI 不得代为输入凭据**（§11.2）。
- [x] 【本地】说明用途并取得用户同意后，创建/关联远端仓库并确认 Actions 已启用；不擅自修改仓库设置。
- [x] 【本地】创建 feature 分支，不直接在 `main` 上开发。
- [x] 编写 `ci.yml` 骨架：`lint`（`ubuntu-24.04`，`cargo fmt --check` + `clippy`）、`test-linux`（`ubuntu-24.04`）、`test-windows`（`windows-latest`）三个 job；固定 runner 版本与 action 版本；配置 `concurrency` + `cancel-in-progress`、`timeout-minutes`、`paths-ignore`、`workflow_dispatch`、Rust 缓存；npm 缓存待 GUI job 启用后加入。
- [x] Windows job 中分别用 `shell: powershell`（5.1）与 `shell: pwsh`（7）打印版本号，确认双版本可区分调用（§11.5）。
- [x] 【本地】推送并用 `gh run watch --exit-status` 确认首次运行绿灯；失败时用 `gh run view <id> --log-failed` 定位，遵守 §11.3 的迭代规则（同一根因 3 次未解决即停下汇报）。**这一步是本项目唯一的验证手段跑通的标志**：在它绿灯之前，任何 Rust 代码都无法被验证。
- [x] 在 `docs/ci-workflow.md` 中记录：workflow 职责划分、runner 版本锁定理由、常用 `gh` 命令、迭代规则与授权边界。
- [x] 提交 `ci: bootstrap github actions verification loop`。

### Task 2：实现模型、错误和序列化

**Files:**
- Create: `crates/aliasmgr-core/src/model.rs`
- Create: `crates/aliasmgr-core/src/error.rs`
- Modify: `crates/aliasmgr-core/src/lib.rs`
- Test: `crates/aliasmgr-core/src/model.rs` 内单元测试

- [x] 先写 `AliasRecord` 默认值、JSON 往返、枚举未知值拒绝的失败测试。
- [x] 实现 `AliasRecord`、`TargetType`、`ShellKind`、`Conflict`、`ShellConfig`、`SyncReceipt`（含 per-shell 结果）、`ShellState`、`ManagedNameSet`（当前托管名 + tombstone）、`PathMode`、`PathOrigin`、`record_checksum` 和 `revision` 字段。
- [x] 不实现 `sort_index`（已从设计中移除，排序由查询期字段决定）。
- [x] 使用 Serde 明确 JSON 字段名，时间统一 RFC3339，UUID 使用字符串。
- [x] 【CI】在 GitHub Actions 上执行 `cargo test -p aliasmgr-core model`，预期全部通过。
- [x] 提交 `feat: add alias domain model`。

### Task 3：实现名称校验和冲突数据结构

**Files:**
- Create: `crates/aliasmgr-core/src/validation.rs`
- Test: `crates/aliasmgr-core/src/validation.rs`

- [x] 先写合法名称 `cm`、`copy-mv`、`build_cam`、`g1` 和非法名称 `my alias`、`a=b`、`hello;world`、`$cmd`、超长名称的参数化测试。
- [x] 实现 `validate_alias_name(name: &str)`，使用跨平台正则、非空检查和 64 字符上限。
- [x] 实现 `validate_alias(alias: &AliasRecord)`，校验 Shell 非空、目标程序非空。
- [x] 实现 `{{args}}` 占位符校验：最多一个、`pass_args = false` 时不允许出现、`{{{{args}}}}` 转义，违规返回 `InvalidArgTemplate`。
- [x] 实现高级模式拒绝：`advanced_shell_mode = true` 或 `target_type = RawShellCommand` 返回 `AdvancedModeUnsupported`（见 §0.3.1、§3.3）。
- [x] 实现 PowerShell 保留名称清单与 `NameReserved` 判定（`ReadOnly`/`Constant` 内置 alias）。
- [x] 实现大小写折叠冲突检查入口（目标 Shell 含 PowerShell 时生效）。
- [x] 【CI】在 GitHub Actions 上执行 `cargo test -p aliasmgr-core validation`，预期全部通过。
- [x] 提交 `feat: add alias validation`。

### Task 4：实现 SQLite 存储和迁移

**Files:**
- Create: `crates/aliasmgr-core/src/migrations/0001_initial.sql`
- Create: `crates/aliasmgr-core/src/migrations/mod.rs`
- Create: `crates/aliasmgr-core/src/storage.rs`
- Modify: `crates/aliasmgr-core/src/lib.rs`
- Test: `crates/aliasmgr-core/src/storage.rs`

- [x] 先写内存数据库测试：创建表、插入、按 ID 读取、按名称读取、更新、删除和事务回滚。
- [x] 实现迁移执行器：按 `NNNN_name.sql` 顺序在单事务内执行，维护 `PRAGMA user_version` 与 `schema_migrations(version, applied_at, checksum)`。
- [x] 先写 `SchemaTooNew` 测试：`user_version` 大于程序已知最大编号时拒绝打开并不做任何写入。
- [x] 迁移前创建数据库文件备份到 `backups/db/`，迁移失败即恢复。
- [x] 设置连接 PRAGMA：`journal_mode = WAL`、`foreign_keys = ON`、`busy_timeout = 5000`、`synchronous = FULL`；检测到不可靠文件系统时降级为 `DELETE` 并发出 `UnreliableFilesystem`。
- [x] 实现 `Database::open(path)`、`migrate()`、`insert_alias()`、`get_alias()`、`list_aliases()`、`update_alias()`、`delete_alias()`。
- [x] 实现 `shell_state`、`retired_names`、`overridden_definitions` 的读写：`upsert_shell_state()`、`retire_name()`、`managed_name_set(shell)`、`prune_retired_names()`、`record_override()`。
- [ ] 维护 `name_folded` 列，并实现大小写折叠冲突查询（面向 PowerShell 的记录）。
- [ ] 所有 JSON 字段通过 Serde 编解码，布尔值使用 SQLite integer 映射，时间使用 RFC3339。
- [ ] 实现唯一名称冲突到 `AliasConflict` 的错误映射，并区分“同名”与“大小写冲突”两种消息。
- [x] 【CI】在 GitHub Actions 上执行 `cargo test -p aliasmgr-core storage`，预期全部通过。
- [x] 提交 `feat: add transactional sqlite storage`。

### Task 5：实现搜索和排序

**Files:**
- Create: `crates/aliasmgr-core/src/search.rs`
- Modify: `crates/aliasmgr-core/src/storage.rs`
- Test: `crates/aliasmgr-core/src/search.rs`

- [x] 先写完全匹配、前缀、子串、非连续匹配和目标字段匹配测试，断言 §8.1 表格中的具体权重值。
- [x] 实现 `SearchQuery`（含 `limit`，默认 50，0 表示不限）、`SortField`、`SearchResult` 和稳定评分排序。
- [x] 支持名称、目标、脚本路径、描述、标签、Shell 和目标类型字段。
- [x] 评分权重以集中常量定义，便于测试与文档同步。
- [x] 在 SQLite 查询后进行可测试的评分排序，确保同分按名称升序、再按 `updated_at` 降序。
- [x] 【CI】在 GitHub Actions 上执行 `cargo test -p aliasmgr-core search`，预期全部通过。
- [x] 提交 `feat: add alias search and sorting`。

### Task 6：实现配置路径、权限和文件锁

**Files:**
- Create: `crates/aliasmgr-core/src/config.rs`
- Create: `crates/aliasmgr-core/src/lock.rs`
- Modify: `crates/aliasmgr-core/src/lib.rs`
- Test: `crates/aliasmgr-core/src/config.rs`

- [x] 先写解析优先级测试：`--config-dir` > `ALIASMGR_CONFIG_DIR` > 平台默认（XDG / LOCALAPPDATA）。
- [x] 实现 `AppPaths::discover()`、`ensure_directories()` 和 `generated_path(shell)`（PowerShell 5.1 与 7 返回不同文件名）。
- [x] 实现 `config.toml` 的加载与默认值，覆盖 §2.4 的备份/日志/tombstone 保留策略。
- [x] 实现 Shell 配置路径覆盖项（`shells.<shell>.profile_path` / `rc_path`），供解析失败时的用户指定与 CI 临时 Profile 注入使用（§4.2、§11.7）。
- [x] 实现备份与日志轮转（按份数与 `max_total_bytes`），保证不删除本次操作刚创建的备份。
- [x] 实现全局可写目录检测，为 §5.4 的 `UnsafeTargetLocation` 提供判定函数（Linux other-writable 位、Windows ACL）。
- [x] 实现跨平台全局锁：默认 10 秒超时返回 `LockTimeout`，锁文件记录 PID 与启动时间，仅在持有进程确认不存在时接管，不删除其他活跃进程的锁文件。
- [x] 【CI】在 GitHub Actions 上分别执行 `cargo test -p aliasmgr-core config` 与 `cargo test -p aliasmgr-core lock`（不要在一条命令里传两个过滤参数）。
- [x] 提交 `feat: add platform paths and config lock`。

---

## 14. 阶段二：执行器、Shell 生成和同步

### Task 7：实现结构化执行器

**Files:**
- Create: `crates/aliasmgr-core/src/executor.rs`
- Modify: `crates/aliasmgr-core/src/model.rs`
- Test: `crates/aliasmgr-core/src/executor.rs`

- [x] 先写 native、Python、PowerShell、BAT/CMD、JAR、切换目录的参数数组测试.
- [x] 实现 `Executor` trait 和执行器选择函数。
- [x] 实现 `{{args}}` 占位符展开：无占位符时追加到末尾；有占位符时做元素级替换；只做数组元素替换，不做元素内字符串插值。
- [x] `TargetType::Batch` 使用**独立执行器与独立转义器**，不复用 native exe 逻辑；对参数中的 `%`、`!`、`&`、`|`、`^`、`<`、`>` 转义或拒绝（见 §3.5）。
- [x] 实现工作目录与环境变量的参数化表示（子 shell / `env` 前缀 / `Push-Location`），环境变量值不进入 `Debug` 输出与日志。
- [x] 确保固定参数和用户参数逻辑上保持数组边界，不实现整行命令拼接。
- [x] 对缺失目标提供显式 `TargetMissing`，由调用层决定阻止或允许保存。
- [x] 【CI】在 GitHub Actions 上执行 `cargo test -p aliasmgr-core executor`，预期全部通过。
- [x] 提交 `feat: add structured command executors`。

### Task 8：实现 Bash/Zsh 适配器

**Files:**
- Create: `crates/aliasmgr-core/src/shell.rs`
- Create: `crates/aliasmgr-core/src/shells/mod.rs`
- Create: `crates/aliasmgr-core/src/shells/common.rs`
- Create: `crates/aliasmgr-core/src/shells/bash.rs`
- Create: `crates/aliasmgr-core/src/shells/zsh.rs`
- Test: `crates/aliasmgr-core/src/shells/bash.rs`
- Test: `crates/aliasmgr-core/src/shells/zsh.rs`

- [x] 先写函数优先、简单 alias 优化、工作目录、环境变量、`{{args}}` 中间插入、名称抢占和托管名称清理的精确字符串测试。
- [x] 实现 POSIX 单引号转义，确保路径中的单引号不会改变生成代码含义。
- [x] 默认生成函数；只有满足 §3.2 全部“简单条件”才生成 `alias name='...'`，复杂命令使用 `command ... "$@"`。
- [x] 实现 `render_preemption()`：每个函数定义前输出独立一行 `unalias <name> 2>/dev/null`，前后加空行确保它与函数定义处于不同解析单元（否则交互式 Shell 的 alias 展开会先发生并导致 `syntax error`）。
- [x] 实现 `render_cleanup()`：按“当前托管名 ∪ 未过期 tombstone”清理，alias 用 `unalias`、function 用 `unset -f`，并在清理前用 `declare -f` / `alias` 取值与内联指纹表比对，不一致则跳过并计入跳过清单。
- [ ] 在生成文件头部写入 `revision`、`file_checksum`、`managed` 与 `retired` 清单（`file_checksum` 计算时排除该行本身）。
- [x] 工作目录使用 `( cd -- '<dir>' && command ... )` 子 shell；环境变量使用 `env VAR=value` 前缀，不使用 `export`。
- [ ] 实现 Bash/Zsh 配置路径、加载块**追加到文件末尾**、幂等检测、标记块删除和 `bash -n`/`zsh -n` 检查；加载块中写入已解析的绝对路径。
- [ ] 实现 RC 文件为 symlink 时就地修改目标文件的逻辑（保持 symlink 不被原子替换掉），并对不可信链路返回 `UnsafePath`。
- [ ] 读写用户 RC 文件时保留原有行尾风格。
- [x] 【CI】在 GitHub Actions 上分别执行 `cargo test -p aliasmgr-core shells::bash` 与 `cargo test -p aliasmgr-core shells::zsh`。
- [ ] 【CI】在安装 Bash/Zsh 的 Linux runner 执行真实语法检查，并补一个 oh-my-zsh 环境下的抢占测试。
- [x] 提交 `feat: generate bash and zsh aliases`。

### Task 9：实现 PowerShell 适配器

**Files:**
- Modify: `crates/aliasmgr-core/src/shells/mod.rs`
- Create: `crates/aliasmgr-core/src/shells/powershell.rs`
- Test: `crates/aliasmgr-core/src/shells/powershell.rs`

- [x] 先写函数优先、`Set-Alias` 简单目标、固定参数、`{{args}}` 中间插入、工作目录、环境变量、参数透传、名称抢占、托管名称清理和覆盖恢复提示的生成测试。
- [x] 实现 PowerShell 字符串字面量转义，Windows 路径使用字面量传入调用运算符。
- [x] 默认生成 `function global:name { & ... @args }`；只有满足 §3.2 全部“简单条件”才生成 `Set-Alias -Scope Global -Force`。
- [x] 实现 `render_preemption()`：定义前输出 `Remove-Item -LiteralPath Alias:\<name> -Force -EA SilentlyContinue` 与 `Remove-Item -LiteralPath Function:\<name> -Force -EA SilentlyContinue`。**这是必需项**：PowerShell 名称解析顺序为 Alias → Function，不清理同名 alias 时函数永不生效，且语法检查会通过（见 §3.4）。
- [ ] 用真实 PS 5.1/7 测试覆盖内置 alias 抢占：创建名为 `ls` 的托管别名后 `Get-Command ls` 必须返回托管 Function。
- [ ] 实现 `ReadOnly`/`Constant` 内置 alias 的保留名称判定，返回 `NameReserved`，在创建阶段拒绝而非运行时静默失败。
- [ ] 实现 `render_cleanup()`：按“当前托管名 ∪ 未过期 tombstone”清理 `Alias:\` 与 `Function:\`，清理前用 `(Get-Item Function:\<name>).Definition` 与内联指纹比对，不一致则跳过。
- [ ] 生成文件头部写入 `revision`、`file_checksum`、`managed` 与 `retired` 清单。
- [ ] 按目标版本选择原生命令参数传递策略（PS 5.1 / 7.0–7.2 旧式拼接、7.3+ `Standard`），并把无法安全传递的参数形态写入 `docs/limitations.md`；不得为绕过限制改用字符串求值（见 §3.5）。
- [ ] 通过实际执行 `powershell.exe -NoProfile -NonInteractive -Command "$PROFILE.CurrentUserAllHosts"` 与 `pwsh` 等价命令解析 Profile 路径；失败时回退 `SHGetKnownFolderPath(FOLDERID_Documents)`；两者皆失败返回 `ConfigNotFound`。必须覆盖 OneDrive 重定向 Documents 的场景。
- [ ] 支持 `config.toml` 中的 `profile_path` 覆盖，并优先于自动解析；CI 依赖该能力做隔离测试（§11.7）。
- [ ] PowerShell 5.1 与 7 生成**独立文件**：`powershell5.ps1` 使用 UTF-8 BOM，`powershell7.ps1` 使用 UTF-8 无 BOM；修改用户 Profile 时保留其原有编码与行尾。
- [ ] 实现 Parser API 语法检查；实现 ExecutionPolicy 诊断：检出 `Restricted`（Profile 完全不执行）时输出用户自行执行的 `Set-ExecutionPolicy -Scope CurrentUser RemoteSigned` 指引，检出 `AllSigned`/GroupPolicy 锁定/ConstrainedLanguage 时报告 `ExecutionPolicyBlocked` 并标注不支持；不修改策略、不使用 `-ExecutionPolicy Bypass` 伪装成功。
- [x] 【CI】在 GitHub Actions 上执行 `cargo test -p aliasmgr-core shells::powershell`，预期全部通过。
- [ ] 【CI】在 Windows runner 上运行真实 Parser 检查与 PS 5.1/7 双版本参数矩阵（PS 7 已预装，无需安装）。
- [x] 提交 `feat: generate powershell aliases`。

### Task 10：实现配置加载块管理

**Files:**
- Modify: `crates/aliasmgr-core/src/shells/common.rs`
- Modify: `crates/aliasmgr-core/src/shells/bash.rs`
- Modify: `crates/aliasmgr-core/src/shells/zsh.rs`
- Modify: `crates/aliasmgr-core/src/shells/powershell.rs`
- Test: `crates/aliasmgr-core/src/shells/common.rs`

- [x] 先写空配置创建、首次插入、重复插入、只删除标记块、缺失结束标记拒绝修改的测试。
- [x] 实现标记块解析器，要求开始和结束标记成对出现且最多一个完整块。
- [x] 加载块**始终追加到文件末尾**；检测到已有加载块不在末尾时返回可报告状态（供 `doctor` 提示“可能被后续配置覆盖”），提供显式的“移动到末尾”操作但不自动执行。
- [x] 加载块中写入已解析的绝对生成文件路径，变量名使用 `__aliasmgr_generated_file` / `__AliasMgrGeneratedFile`，Bash 用 `unset -v` 清理。
- [x] 写入前创建时间戳备份到 `backups/rc/`，按 §2.4 的保留策略清理。
- [ ] 对被手工修改的生成文件比较 `shell_state.file_checksum` 与磁盘内容 checksum，返回需要用户决策的状态。
- [ ] 将“当前托管名 ∪ 未过期 tombstone”清单写入生成文件，并让清理逻辑在定义之前执行；验证删除/禁用/改名后 reload 不残留旧定义（这是 §5.1.1 的核心，必须有真实 Shell 回归测试）。
- [ ] 实现定义指纹比对，避免误删用户在会话中自行重建的同名定义；跳过项进入跳过清单。
- [ ] 强制覆盖已有 alias/function 时把原定义快照写入 `overridden_definitions`；无法解析时置 `recoverable = 0`，并在 CLI/GUI 中提示用户。
- [x] 【CI】在 GitHub Actions 上执行 `cargo test -p aliasmgr-core loader`，预期全部通过。
- [x] 提交 `feat: manage shell loader blocks safely`。

### Task 11：实现原子同步、journal 和回滚

**Files:**
- Create: `crates/aliasmgr-core/src/sync.rs`
- Modify: `crates/aliasmgr-core/src/storage.rs`
- Test: `crates/aliasmgr-core/src/sync.rs`

- [x] 先写生成临时文件、语法检查失败不替换、替换失败恢复、数据库事务回滚和 journal 状态测试。
- [x] 实现 `SyncCoordinator::apply()`：锁定、备份、生成、检查、原子替换、提交和释放锁；返回 per-shell 的 `SyncReceipt`。
- [x] journal 记录 `revision_from`、`revision_to` 与 `backups_json`（多个备份路径的数组，覆盖每个 RC/Profile 与每个生成文件）。
- [x] 实现启动恢复 `recover_pending_operations()`：依据 `revision_from`/`revision_to` 判断前滚（数据库已提交则重建生成文件）或回滚（未提交则按 `backups_json` 恢复并清理临时文件）。
- [x] 每个 Shell 独立生成和替换，成功即更新该 Shell 的 `shell_state`；单 Shell 失败不回滚其他 Shell 的成功替换、不回滚数据库。
- [ ] 实现 `shell_state.status` 计算（`ok`/`stale`/`failed`/`loader_missing`/`unknown`），供 `doctor` 报告派生文件过期。
- [x] 写入 `retired_names` 并在同步成功后执行 `prune_retired_names()`。
- [ ] 先写多 Shell 部分失败测试：PowerShell 检查失败、Bash 成功时，`shell_state` 分别为 `failed` 与 `ok`，且数据库 revision 已递增。
- [x] 【CI】在 GitHub Actions 上执行 `cargo test -p aliasmgr-core sync`，预期全部通过。
- [x] 提交 `feat: add atomic shell synchronization and rollback`。

### Task 12：实现 Shell 检测和冲突检测

**Files:**
- Create: `crates/aliasmgr-core/src/detection.rs`
- Modify: `crates/aliasmgr-core/src/shell.rs`
- Test: `crates/aliasmgr-core/src/detection.rs`

- [x] 先写显式 Shell 优先、父进程优先于 `$SHELL`、默认 Shell 回退和已安装 Shell 扫描测试。
- [x] 实现 Linux `/proc` 父链读取，并在不可用时安全回退，不把 `$SHELL` 当作当前 Shell 的绝对事实。
- [ ] 实现 Windows `powershell.exe`/`pwsh.exe` 发现和版本区分（含 PS 7 的多版本并存）。
- [x] 实现 PATH 可执行文件冲突报告；Windows 必须按 `PATHEXT` 逐后缀查找，不只查 `.exe`。
- [x] 实现已知内置命令、Cmdlet、内置 alias 与保留名称（`ReadOnly`/`Constant`）的冲突报告，保留名称直接判定为 `NameReserved`。
- [x] 实现用户配置中同名定义的来源定位（文件 + 行号），用于 `doctor` 报告“插件在加载块之后重新定义”。
- [x] 【CI】在 GitHub Actions 上执行 `cargo test -p aliasmgr-core detection`，预期全部通过。
- [x] 提交 `feat: detect shells and report conflicts`。

---

## 15. 阶段三：Linux CLI 和 Windows CLI 能力

### Task 13：实现 CLI 参数和输出层

**Files:**
- Create: `crates/aliasmgr-cli/src/cli.rs`
- Create: `crates/aliasmgr-cli/src/commands.rs`
- Create: `crates/aliasmgr-cli/src/output.rs`
- Create: `crates/aliasmgr-cli/src/exit_code.rs`
- Create: `crates/aliasmgr-cli/src/messages.rs`
- Modify: `crates/aliasmgr-cli/src/main.rs`
- Test: `crates/aliasmgr-cli/tests/parse.rs`

- [ ] 先写命令树解析测试，覆盖 `add`、`remove`、`update`、`find`、`list`、`sync`、`reload`、`doctor`、`shell`、`import`、`export`、`uninstall`。
- [ ] 使用 `clap` 定义全局 `--config-dir`、`--format table|json`、`--no-color`、`--verbose/--quiet`，以及 `--arg` 可重复参数、`--env KEY=VALUE`、`--tag`、`--cwd`、`--shell auto`、`--limit`、`--dry-run` 和 `--yes`。
- [ ] 实现 `exit_code.rs`：按 §6.3 的表格把 `AliasError` 映射为固定退出码，并写一个覆盖表格每一行的测试，防止后续改动漂移。
- [ ] 用户可见文案集中在 `messages.rs`，不散落在各命令实现中。
- [ ] 非 TTY 且缺少 `--yes` 的交互命令返回 `NonInteractive`（退出码 14），不阻塞等待输入。
- [ ] 实现稳定表格列和稳定 JSON 字段，错误输出到 stderr。
- [ ] 【CI】在 GitHub Actions 上执行 `cargo test -p aliasmgr-cli --test parse`，预期全部通过。
- [ ] 提交 `feat: add alias manager cli command model`。

### Task 14：实现增删改查、搜索、启用和禁用

**Files:**
- Modify: `crates/aliasmgr-cli/src/commands.rs`
- Test: `crates/aliasmgr-cli/tests/crud.rs`

- [ ] 先写端到端 CLI 测试，通过 `ALIASMGR_CONFIG_DIR` 指向临时目录：添加 `gs`、获取、查找、列表、更新、改名、禁用、启用、删除。
- [ ] 将所有操作委托给 `aliasmgr-core`，CLI 不直接访问 SQLite 表或生成 Shell 代码。
- [ ] 删除操作默认交互确认，`--yes` 执行非交互删除，并逐 Shell 报告需要重新加载的命令与“当前会话可能仍有旧定义”的提示。
- [ ] 改名时把旧名写入 `retired_names`，并在输出中说明旧名会在下次 reload 时从会话中清除。
- [ ] 【CI】在 GitHub Actions 上执行 `cargo test -p aliasmgr-cli --test crud`，预期全部通过。
- [ ] 提交 `feat: add cli alias lifecycle commands`。

### Task 15：实现 sync、doctor 和 shell 子命令

**Files:**
- Modify: `crates/aliasmgr-cli/src/commands.rs`
- Modify: `crates/aliasmgr-cli/src/output.rs`
- Test: `crates/aliasmgr-cli/tests/diagnostics.rs`

- [ ] 先写 `shell detect` 输出、`shell install`/`uninstall` 幂等、`doctor` 发现缺失文件和语法错误的测试。
- [ ] 实现 `sync` 调用核心同步器，逐 Shell 输出结果；部分失败时返回退出码 10，全部失败按具体错误映射。
- [ ] 实现 `sync --dry-run`：打印将生成的脚本内容与将修改的文件，不做任何写入。
- [ ] 实现 `reload [--print]`：`--print` 输出可被父 Shell 求值的单行 source 命令，供 `eval "$(aliasmgr reload --print)"` 使用（见 §6.2）。
- [ ] 实现 `doctor` 检查：数据库与迁移版本、生成文件存在性、`shell_state` 是否过期（`applied_revision` 落后）、加载块存在性与**是否位于文件末尾**、语法、目标存在性、目标是否位于全局可写目录、权限、`file_checksum`、`.bashrc` 生效性（非交互守卫、login shell 链）、PowerShell ExecutionPolicy（分 5.1/7）、会话清理跳过清单、插件后置覆盖来源。
- [ ] 提供 Bash/Zsh/PowerShell 重新加载命令提示，但不宣称更新了已有父 Shell。
- [ ] 【CI】在 GitHub Actions 上执行 `cargo test -p aliasmgr-cli --test diagnostics`，预期全部通过。
- [ ] 提交 `feat: add cli sync diagnostics and shell management`。

### Task 16：实现导入导出

**Files:**
- Create: `crates/aliasmgr-core/src/transfer.rs`
- Modify: `crates/aliasmgr-cli/src/commands.rs`
- Test: `crates/aliasmgr-cli/tests/transfer.rs`

- [ ] 先写导出再导入的字段保真测试和敏感环境变量过滤测试。
- [ ] 导出包含 `format_version`、`exported_at`、`exported_by_version`；先写 `format_version` 过高被拒绝、缺失被拒绝的测试。
- [ ] 实现 JSON/TOML 导出、冲突策略 `skip|overwrite|rename|ask` 和导入安全报告。
- [ ] 导入相对路径时要求用户选择基准目录或拒绝导入；`advanced_shell_mode`/`RawShellCommand` 记录标记为 unsupported 并跳过。
- [ ] 不执行导入文件中的高级命令；缺失目标、网络路径、全局可写目录目标和危险命令只生成警告或阻止同步。
- [ ] 【CI】在 GitHub Actions 上执行 `cargo test -p aliasmgr-cli --test transfer`，预期全部通过。
- [ ] 提交 `feat: add safe alias import and export`。

### Task 17：实现卸载清理

**Files:**
- Create: `crates/aliasmgr-core/src/uninstall.rs`
- Modify: `crates/aliasmgr-cli/src/commands.rs`
- Test: `crates/aliasmgr-cli/tests/uninstall.rs`

- [ ] 先写保留别名、删除别名、删除数据库/备份可选项、保留用户配置和保护目标文件的测试。
- [ ] 保留模式下移除加载块中对 tombstone 清理逻辑的依赖，确认生成文件可独立 source（原则 11）。
- [ ] 覆盖“用户手工删除程序但未执行清理”的情形：加载块的 `[ -r ]` / `Test-Path` 守卫必须让缺失生成文件时 Shell 启动不报错。
- [ ] 实现 `aliasmgr uninstall --purge-aliases`，只删除 Alias Manager 标记块、生成脚本和用户明确指定的数据。
- [ ] 卸载清理必须把 EXE、BAT、CMD、Python、PowerShell、Shell、JAR 等目标路径视为只读引用，禁止删除、移动或修改这些文件。
- [ ] 保留模式验证生成脚本只依赖目标解释器/程序，不引用 `aliasmgr invoke`。
- [ ] Linux 包卸载文档说明包管理器的 `postrm` 无法交互，不保证弹出 GUI，需引导用户先执行 `aliasmgr uninstall`；Windows 安装器（MSI 自定义动作）调用同一核心清理逻辑并在 UI 中提供保留/删除选项。
- [ ] 【CI】在 GitHub Actions 上执行 `cargo test -p aliasmgr-cli --test uninstall`，预期全部通过。
- [ ] 提交 `feat: add safe uninstall cleanup modes`。

---

## 16. 阶段四：Tauri GUI（MVP 交付物，Linux + Windows）

### Task 18：初始化 Tauri 2 GUI 外壳

**Files:**
- Create: `crates/aliasmgr-gui/src-tauri/Cargo.toml`
- Create: `crates/aliasmgr-gui/src-tauri/src/main.rs`
- Create: `crates/aliasmgr-gui/ui/package.json`
- Create: `crates/aliasmgr-gui/ui/src/main.ts`
- Create: `crates/aliasmgr-gui/ui/src/App.tsx`

- [ ] 实现一个静态页面并调用一个返回版本号的 Rust command。
- [ ] 【CI】在 GitHub Actions 上执行 `cargo tauri build`（编译检查，非 `cargo tauri dev`）与前端测试，预期编译通过且版本 command 的单元测试通过。`cargo tauri dev` 需要图形界面与本地 Rust 工具链，不在 CI 也不在本地执行；应用可启动性由用户下载 artifact 后人工确认（§11.9）。
- [ ] **在 Linux 与 Windows 两个平台上分别验证可启动**（GUI 是 MVP 基础需求，单平台通过不算完成）。
- [ ] 通过 `src-tauri/src/commands.rs` 暴露列表、保存、删除、同步、诊断和导入导出接口，所有命令调用 core。
- [ ] 提交 `feat: initialize tauri gui shell`。

### Task 19：实现列表、搜索和编辑向导

**Files:**
- Create: `crates/aliasmgr-gui/ui/src/components/AliasTable.tsx`
- Create: `crates/aliasmgr-gui/ui/src/components/AliasWizard.tsx`
- Create: `crates/aliasmgr-gui/ui/src/components/SearchBar.tsx`
- Modify: `crates/aliasmgr-gui/ui/src/App.tsx`
- Test: `crates/aliasmgr-gui/ui/tests/aliases.spec.ts`

- [ ] 先写 UI 测试：显示列表、搜索名称、打开向导、拒绝非法名称、拒绝保留名称、选择 Shell、显示预览。
- [ ] 实现状态、别名、目标、类型、Shell、参数透传、更新时间、冲突状态和 per-shell 同步状态（`ok`/`stale`/`failed`）列。
- [ ] 向导分为名称、目标类型、结构化参数（含 `{{args}}` 占位符位置的可视化）、执行环境（工作目录 + 环境变量 + 标签）、Shell、预览和测试确认。
- [ ] 文件选择器返回路径默认规范化为绝对路径，并记录 `path_mode = absolute`、`path_origin = gui_file_picker`；相对路径需用户显式勾选后才可保存。
- [ ] 【CI】在 GitHub Actions 上执行 `npm test` 和 Tauri 类型检查，预期全部通过。
- [ ] 提交 `feat: add alias management gui workflow`。

### Task 20：实现诊断、导入导出和卸载设置页

**Files:**
- Create: `crates/aliasmgr-gui/ui/src/components/DoctorPanel.tsx`
- Create: `crates/aliasmgr-gui/ui/src/components/SettingsPanel.tsx`
- Modify: `crates/aliasmgr-gui/src-tauri/src/commands.rs`
- Test: `crates/aliasmgr-gui/ui/tests/settings.spec.ts`

- [ ] 先写诊断失败展示、备份提示、导入安全报告和卸载保留/删除选项测试。
- [ ] 实现诊断结果分组、生成代码只读预览、重新加载命令复制按钮和清理确认对话框。
- [ ] 设置页覆盖备份轮转、日志轮转、默认 Shell、配置目录、相对路径开关与卸载策略。
- [ ] 展示被覆盖的原定义（`overridden_definitions`）与恢复入口；`recoverable = 0` 时明确标注不可自动恢复。
- [ ] ExecutionPolicy 为 `Restricted` 时在 Windows GUI 首屏给出醒目提示与用户自行执行的命令，不提供“一键修改策略”按钮。
- [ ] 测试运行显示最终参数和副作用警告，普通模式通过 core 的结构化执行接口。
- [ ] 【CI】在 GitHub Actions 上执行 `npm test`、`cargo test --workspace` 和 Tauri build（Linux 与 Windows 各一次），预期全部通过。
- [ ] 提交 `feat: add gui diagnostics and cleanup settings`。

---

## 17. 阶段五：发布、CI 和质量保证（M6，MVP 收尾）

> 原“Task 22：工作目录、环境变量和标签”已删除。按 §0.2 的决策，这三项并入 MVP，分别在 Task 2（模型）、Task 4（存储）、Task 7（执行器）、Task 8/9（生成）、Task 14（CLI）、Task 19（GUI）中实现，因此不需要 `0002_extended_execution.sql` 迁移。

### Task 21：补全 CI 测试矩阵与发布流水线

在 Task 1.1 建立的 `ci.yml` 骨架之上补全全部 job，并按 §11.10 的节奏加入 GUI 与打包。规格见 §11.4–§11.8，本任务只负责落地与验证。

**Files:**
- Modify: `.github/workflows/ci.yml`
- Create: `.github/workflows/integration.yml`
- Create: `.github/workflows/release.yml`
- Modify: `docs/ci-workflow.md`
- Create: `crates/aliasmgr-tests/fixtures/argument-dumper.py`
- Create: `crates/aliasmgr-tests/fixtures/argument-dumper.ps1`
- Create: `crates/aliasmgr-tests/fixtures/argument-dumper.bat`

- [ ] Linux job 安装 §11.5 列出的 apt 依赖（注意 24.04 上是 `libayatana-appindicator3-dev` 而非 `libappindicator3-dev`），运行 Rust 单元/集成测试与 Bash/Zsh 适配器测试。
- [ ] 追加一个安装 oh-my-zsh 的 job，覆盖 §3.4 的名称抢占与加载块位置。
- [ ] Windows job 用 `shell: powershell`（5.1）与 `shell: pwsh`（7）分别运行 Profile 定位、Parser 检查、UTF-8 BOM 校验、名称抢占（`ls` 用例）和 §3.5 完整参数矩阵；不得用 PS 7 代表两个版本。
- [ ] `cargo fmt --check` 只在 Linux 的 `lint` job 执行一次；`clippy` 在两个平台都执行以覆盖 `#[cfg]` 分支。
- [ ] GUI job 在 Linux 与 Windows 上分别执行前端单元测试、类型检查与 Tauri 编译（GUI 属 MVP，不可只测单平台）。
- [ ] 建立 `integration.yml`，把 §11.4 列出的慢速集成测试从 `ci.yml` 迁出，只在 `workflow_dispatch`、`main` 的 push 与核心路径的 PR 上运行。
- [ ] 建立 `release.yml`，仅 tag 或 `workflow_dispatch` 触发；Linux 打包 job 固定使用 `ubuntu-22.04` 以保证 AppImage 的 glibc 兼容性；签名凭据全部来自 GitHub Secrets，仓库内不含证书；不自动创建 Release。
- [ ] 加入退出码表回归测试与 §9.1.1 的加载耗时基准（500/1000 条别名）。
- [ ] 验证 §11.7 的隔离要求：所有测试使用临时 HOME/`XDG_CONFIG_HOME`/`LOCALAPPDATA`/`APPDATA` 与注入的 Profile 路径，runner 的真实用户配置在测试前后无变化（加一个断言步骤）。
- [ ] 落实 §11.8 的日志脱敏：不转储环境变量、不启用 `set -x`、不打印完整 Shell 配置、artifact 仅含脱敏后的生成脚本快照且保留期 7 天。
- [ ] 提交 `ci: add cross-platform shell test matrix`。

### Task 22：完成文档、安装和卸载验收

**Files:**
- Create: `README.md`
- Create: `docs/architecture.md`
- Create: `docs/security.md`
- Create: `docs/testing.md`
- Create: `docs/uninstall.md`
- Create: `docs/exit-codes.md`
- Create: `docs/limitations.md`
- Modify: `docs/ci-workflow.md`
- Create: Windows installer and Linux packaging files

- [ ] 文档说明 Bash/Zsh/Fish 是 Shell，不是终端类型，并统一使用 `alias` 拼写。
- [ ] 文档说明 Profile/RC 加载块、重新加载限制、备份位置、执行策略和卸载保留/删除行为。
- [ ] `docs/exit-codes.md` 与 §6.3 表格逐行一致，并声明退出码含义发布后不再变更。
- [ ] `docs/limitations.md` 记录已知限制：PS 5.1/7.2 原生参数传递缺陷、`.bat` 二次解析、别名不递归解析、只在交互式 Shell 生效、`AllSigned`/ConstrainedLanguage 不支持、名称全局唯一（不支持跨 Shell 同名不同义）。
- [ ] `docs/ci-workflow.md` 补充 §11.9 的能力边界，明确 CI 绿灯不等于 GUI 交互、真实用户配置兼容性与真实终端行为已验证。
- [ ] 在干净 Linux 用户目录和干净 Windows 用户配置中执行安装、创建、重启 Shell、删除、升级和卸载验收。**该项必须在真实机器上人工完成，CI 无法替代（§11.9）。**
- [ ] 验证卸载不删除用户未托管内容，保留模式不依赖 Alias Manager 可执行文件。
- [ ] 提交 `docs: document architecture security and lifecycle`。

### Task 23：最终安全和发布检查

**Files:**
- Modify: 受检查发现影响的源文件、测试和文档
- Create: `docs/release-checklist.md`

- [ ] 用搜索确认普通生成路径没有 `eval`、`Invoke-Expression` 或任意字符串求值调用；`reload --print` 是唯一例外且需在清单中说明理由。
- [ ] 确认 `validate_alias` 仍然拒绝 `advanced_shell_mode` 与 `RawShellCommand`。
- [ ] 检查所有生成文件写入点都经过锁、备份、语法检查和原子替换。
- [ ] 检查日志、导出、错误信息不会泄露密码、Token、API Key 或敏感参数（含环境变量值）。
- [ ] 【CI】触发全量 workflow：格式化、静态检查、单元测试、集成测试、打包测试；安装/卸载的真机验收由用户人工完成（§11.9）。
- [ ] 只有所有检查通过后提交 `release: verify alias manager mvp`。

---

## 18. 阶段六：扩展能力（M7，MVP 之后）

### Task 24：Fish 和 POSIX Shell 适配器

**Files:**
- Create: `crates/aliasmgr-core/src/shells/fish.rs`
- Create: `crates/aliasmgr-core/src/shells/posix.rs`
- Test: 对应适配器单元测试和 `crates/aliasmgr-tests/tests/fish.rs`

- [ ] 先写 Fish `$argv` 参数透传、函数、配置路径和 `fish -n` 测试。
- [ ] 实现 Fish 独立语法，不复用 Bash 函数文本；Fish 的名称抢占与清理需按 `functions -e` / `abbr` 语义单独实现。
- [ ] 实现 POSIX 有限兼容模式，只生成可移植的函数/alias 子集。
- [ ] 执行 Fish/POSIX 集成测试，预期语法和参数测试通过。
- [ ] 提交 `feat: add fish and posix shell adapters`。

### Task 25：安全扫描和现有配置导入

**Files:**
- Create: `crates/aliasmgr-core/src/import_scan.rs`
- Modify: `crates/aliasmgr-core/src/transfer.rs`
- Test: `crates/aliasmgr-tests/tests/import_scan.rs`

- [ ] 先写简单 Bash alias、简单 PowerShell AST 和复杂动态配置拒绝执行的测试。
- [ ] 实现纯解析扫描，不执行 `.bashrc`、`.zshrc` 或 Profile。
- [ ] 对函数、管道、重定向、动态变量和 `Invoke-Expression` 输出人工确认标记。
- [ ] 执行导入安全集成测试，预期未知结构不会自动导入或执行。
- [ ] 提交 `feat: scan shell configuration safely`。

### Task 26：WSL、CMD、Git Bash、Nushell 和插件边界

**Files:**
- Create: `docs/plugin-api.md`
- Modify: `crates/aliasmgr-core/src/shell.rs`
- Modify: `crates/aliasmgr-core/src/executor.rs`
- Test: 各适配器发现和拒绝不支持场景的测试

- [ ] 先写检测到但未安装/未启用适配器时的稳定结果测试。
- [ ] 为新 Shell 明确 `ShellAdapter` 和 `Executor` 接口，不把平台分支散落到 CLI/GUI。
- [ ] 新适配器必须实现 `render_preemption` 与 `render_cleanup`，并通过与 Bash/PowerShell 相同的抢占与 tombstone 契约测试。
- [ ] 仅在每个 Shell 的参数边界、配置路径和语法检查可验证后启用，不为未验证 Shell 提供“已支持”状态。
- [ ] 执行适配器契约测试，预期所有适配器共享名称校验、加载块和错误语义。
- [ ] 提交 `feat: define extensible shell adapter boundaries`。

---

## 19. 交付检查清单

### 核心和存储

- [ ] Rust workspace 可在 Linux/Windows 构建。
- [ ] SQLite 迁移、事务、操作日志和回滚测试通过。
- [ ] 别名名称、目标、Shell 和冲突校验完成。
- [ ] 搜索、模糊评分、排序和 JSON 序列化稳定。

### Shell 和同步

- [ ] Bash、Zsh、PowerShell 5.1、PowerShell 7 适配器通过真实语法检查。
- [ ] 默认优先使用函数包装器；只有满足全部简单条件时才使用原生 alias/`Set-Alias`。
- [ ] 带固定参数、工作目录、环境变量或参数透传的命令使用函数和安全参数透传。
- [ ] 生成文件独立于 Alias Manager 可执行文件。
- [ ] 定义前清理同名 alias/function，已存在同名 alias（含 PowerShell 内置 `ls`/`cp`/`gc` 等）时托管定义仍生效。
- [ ] `ReadOnly`/`Constant` 保留名称在创建阶段被 `NameReserved` 拒绝，而非运行时静默失效。
- [ ] SQLite 是唯一事实来源，生成文件包含 revision、`file_checksum`、`managed` 与 `retired` 清单。
- [ ] tombstone 机制可清除已删除/改名/禁用别名在会话中的残留定义；指纹比对保护用户自行重建的同名定义。
- [ ] RC/Profile 只包含带标记的加载块，且加载块位于文件末尾。
- [ ] RC/Profile 为 symlink 时就地修改目标文件，symlink 未被替换；编码与行尾保持不变。
- [ ] PowerShell 5.1 与 7 使用独立生成文件，5.1 为 UTF-8 BOM、7 为无 BOM。
- [ ] PowerShell Profile 路径通过实测获取，OneDrive 重定向场景验证通过。
- [ ] 删除、禁用或覆盖后能清理 Alias Manager 已加载名称，并提示当前会话重新加载和原定义恢复风险。
- [ ] `{{args}}` 占位符支持中间插入，重复/非法组合被 `InvalidArgTemplate` 拒绝。
- [ ] `.bat`/`.cmd` 使用独立转义器，危险字符被转义或拒绝。
- [ ] 备份、锁、operation journal（含 `revision_from/to`、`backups_json`）、`shell_state`、`record_checksum`/`file_checksum`、原子替换和恢复有效。
- [ ] 多 Shell 部分失败时 `shell_state` 准确、退出码为 10、`doctor` 能报告过期。
- [ ] 迁移执行器、`user_version`、`schema_migrations` 与 `SchemaTooNew` 拒绝逻辑通过测试。
- [ ] 相对路径只有用户显式启用时可用，GUI 选择路径默认保存为绝对路径并记录 `path_mode`/`path_origin`。
- [ ] 全局可写目录中的目标触发 `UnsafeTargetLocation` 警告并需显式确认。
- [ ] 卸载不会删除别名引用的用户目标文件。
- [ ] 导入必须经过预览、验证、安全报告和明确确认；`format_version` 校验生效。
- [ ] 加载耗时基准（§9.1.1）在 500/1000 条别名下达标并记录。

### CLI 和 GUI

- [ ] CLI 增删改查、搜索、排序、同步、`reload`、诊断、导入导出和卸载完成。
- [ ] CLI 支持交互与非交互模式（非 TTY 返回 `NonInteractive`）、§6.3 的稳定退出码和 JSON 输出。
- [ ] `--config-dir` / `ALIASMGR_CONFIG_DIR` 覆盖生效，测试不写入真实用户目录。
- [ ] `sync --dry-run` 不产生任何写入。
- [ ] GUI 在 **Linux 与 Windows 双平台**完成主列表、向导、预览、测试、诊断和设置（MVP 基础需求）。
- [ ] GUI 不复制核心业务逻辑。

### 安全和生命周期

- [ ] 普通模式未使用 `eval` 或 `Invoke-Expression`；`advanced_shell_mode` 与 `RawShellCommand` 被 validation 拒绝。
- [ ] 配置权限、符号链接策略、敏感变量和日志脱敏通过检查。
- [ ] PowerShell 执行策略只诊断、不自动绕过；`Restricted` 场景有正确指引。
- [ ] `.bashrc` 生效性（非交互守卫、login shell 链、oh-my-zsh 覆盖）诊断通过。
- [ ] 卸载可选择保留或删除，且不破坏用户其他配置；程序被手工删除后 Shell 启动不报错。
- [ ] 已打开 Shell 的重新加载限制被准确提示。

### CI 与远程验证

- [ ] `ci.yml`、`integration.yml`、`release.yml` 三个 workflow 职责清晰且均可成功运行。
- [ ] runner 版本与 action 版本已固定，未使用浮动的 `ubuntu-latest`/`latest`。
- [ ] Linux 依赖包名与目标发行版匹配（24.04 用 `libwebkit2gtk-4.1-dev` + `libayatana-appindicator3-dev`）。
- [ ] AppImage 打包在 `ubuntu-22.04` 上执行，glibc 兼容性验证通过。
- [ ] PowerShell 5.1 与 7 通过 `shell: powershell` / `shell: pwsh` 分别测试，未用 7 代表两者。
- [ ] `concurrency` 取消、`fail-fast: false`、`timeout-minutes`、缓存与 `paths-ignore` 均已配置。
- [ ] CI 测试全程隔离：临时 HOME/`XDG_CONFIG_HOME`/`LOCALAPPDATA`/`APPDATA` 与注入的 Profile 路径；有断言证明 runner 真实用户配置未被修改。
- [ ] 日志与 artifact 已脱敏：无环境变量转储、无 `set -x`、无完整 Shell 配置输出、无凭据、artifact 保留期 7 天。
- [ ] 签名证书不在仓库中，仅通过 GitHub Secrets 注入且只在 tag 触发的 `release.yml` 中使用。
- [ ] `docs/ci-workflow.md` 记录了 workflow 划分、`gh` 迭代规则、授权边界与 §11.9 的能力边界。

### 发布

- [ ] Linux 和 Windows CI 全部通过（含 GUI job）。
- [ ] 安装、升级、恢复和卸载在干净环境验证。
- [ ] README、架构、安全、测试、退出码、已知限制和卸载文档完成。
- [ ] 发布清单逐项签字确认后才生成安装包。
