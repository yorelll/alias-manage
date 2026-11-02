# CLAUDE.md — Alias Manager 协作规则

本文件定义本项目多 agent 协作的固定角色、流程与测试边界。所有子 agent 必须遵守。

## 角色与职责

### 实现 Agent（Agent A）
- 只负责实现任务，在**独立 git worktree** 中工作，不直接修改主 checkout。
- 必须自行运行与改动相关的单元/集成测试，并提交 commit。
- 提交后报告：分支名、commit SHA、改动文件、实际执行的测试命令与结果、偏差与已知缺口。
- 不得 cherry-pick、merge、push 到主分支；由主控（主会话）负责 cherry-pick。
- **必须编写尽可能详细的测试代码**，不能只验证“能编译/能跑”：
  - 正常路径（happy path）与至少一组端到端路径；
  - 边界条件：空值、极值、长度上限、前缀/兄弟目录、根路径、大小写、尾部分隔符、符号链接/联接点、缺失文件/目录、跨平台路径、Unicode/空格/引号；
  - 错误路径：无效输入、权限/只读/缺失目标、冲突、迁移失败、回滚、并发/锁；
  - 每个修复或新功能都要有能区分修复前/后的回归测试（必要时说明修复前会失败的证据）；
  - 覆盖 core 单元测试、CLI 集成测试、GUI Rust/前端测试中与改动相关的所有层；跨 Shell 行为至少覆盖生成文本与可执行路径。

### Review Agent（Agent B）
- **只做 code review，不修改任何代码**，不提交、不 cherry-pick。
- 审查维度：正确性、安全性（命令注入/路径匹配/转义）、数据迁移与兼容性、CLI/GUI 契约、测试真实性与覆盖、文档准确性、回归与越界改动。
- 输出按严重度排序的 findings（blocker/major/minor/nit），每条包含 `file:line`、缺陷一句话、具体失败场景、具体修复建议。
- 只有确认没有需要修改的问题时，才输出 `NO FINDINGS`；不得为显得认真而制造 nit。

### 功能测试 Agent（Agent C）
- 只负责在真实 Shell 中做功能/验收测试，不修改产品代码。
- 在 B 输出 `NO FINDINGS` 后才启动；若 B 仍有 findings，先回到 A 修复并复审。
- 测试发现缺陷时，记录证据并回传给 A 修复；修复后由 B 复审、C 重测，循环至通过。

## 工作流（循环）

1. A 在 worktree 实现并提交。
2. 主控 cherry-pick A 的 commit 到主分支。
3. B 审查该 commit，输出 findings 或 `NO FINDINGS`。
4. 有 findings → 回传 A 修复 → 新 commit → cherry-pick → B 复审；循环直到 `NO FINDINGS`。
5. B 无意见 → 派 C 执行功能测试。
6. C 发现缺陷 → 回传 A 修复 → B 复审 → C 重测；循环直到通过。

## 功能测试环境

- **PowerShell 5.1 与 PowerShell 7（pwsh）**：在本机（Windows 11）测试。
- **Bash 与 Zsh**：通过 SSH 远程测试，主机配置已存在于 SSH config：

  ```sshconfig
  Host guest_homeserver
    HostName 192.168.3.36
    User guest
    ForwardAgent yes
    ForwardX11 yes
    ForwardX11Trusted yes
    IdentityFile C:\Users\lawrence\.ssh\id_rsa
  ```

  连接方式：`ssh guest_homeserver`。不得修改该主机的 SSH 配置、环境变量、注册表或系统级设置。
- 需要把二进制/脚本送到远端时，复制到远端临时目录（如 `/tmp/aliasmgr-test-$$/`），测试结束立即删除。

## 硬性约束（所有 agent）

1. **禁止修改环境变量或注册表**（本机与远端均同）。不得用 `setx`、`setx /M`、`[Environment]::SetEnvironmentVariable(..., "User"/"Machine")`、`reg add`、修改 `~/.bashrc`/`~/.zshrc`/PowerShell profile 等方式改变测试环境。
2. **禁止修改真实用户配置**：不得写入真实 `%LOCALAPPDATA%\alias-manager`、`%APPDATA%`、真实 `.bashrc`/`.zshrc`/PowerShell Profile。所有测试必须使用隔离配置根（`--config-dir <临时目录>` 或 `ALIASMGR_CONFIG_DIR=<临时目录>`）。
3. **环境保持干净、无残留**：测试后清理临时目录、上传文件、后台进程、锁文件、监听端口与 SSH 会话；保留必要的证据日志，但不保留敏感内容。
4. **只读原则**：ExecutionPolicy、系统策略、主机配置只报告不修改；“让测试通过”不得以改动环境为代价。
5. **敏感信息**：不得在报告、日志、截图路径中保留 Token、密码、API Key、完整 Profile、环境变量转储；用户名/机器名/临时路径需脱敏。

## 测试证据要求

每个测试项必须有完整、可复核的证据，不能只写“通过”：

- 测试 ID 与来源（参考 `docs/task21-2/test-matrix.md` 的 `L-xxx`、`PS51-xxx`、`PS7-xxx`、`B-/Z-xxx` 等编号）。
- 平台、架构、Shell 及版本、应用/commit/SHA256、脱敏后的配置根。
- 实际执行的完整命令。
- 实际输出片段、日志/摘要文件路径、退出码。
- 结果枚举，只能使用：`PASS | FAIL | BLOCKED | EXPECTED-LIMITATION | NOT-APPLICABLE`。
- `FAIL`/`BLOCKED`/`EXPECTED-LIMITATION` 必须附原因与复现步骤。

参考与复用：

- 验收流程与证据规则：`docs/task21-2/README.md`、`docs/task21-2/test-matrix.md`、`docs/task21-2/feedback-template.md`
- Shell 专项：`docs/task21-2/02-shell/`、`docs/task21-2/bash-zsh-user-config.md`、`docs/task21-2/linux-clean-machine.md`、`docs/task21-2/windows-powershell51.md`、`docs/task21-2/windows-powershell7.md`
- 优先复用现有脚本：`scripts/verify-cli-windows.ps1`、`scripts/verify-powershell51.ps1`、`scripts/verify-powershell7.ps1`、`scripts/verify-cli-linux.sh`、`scripts/verify-bash-linux.sh`、`scripts/verify-zsh-linux.sh`

不得根据 CI 结果填写人工验收结果；CI 只能作为自动化边界的补充证据。

## 提交与归属

- commit message 使用 conventional 前缀；结尾附：

  ```
  Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
  ```

- PR 描述结尾附：

  ```
  🤖 Generated with [Claude Code](https://claude.com/claude-code)
  ```

## CI 验证

- 远程 GitHub Actions 验证可用 `gh.exe`（或 `gh`）监控：`gh run list`、`gh run watch <id>`、`gh run view <id> --log`、`gh run download <id>`。
- CI 结果只能作为自动化边界的证据，不得用来填写 `docs/task21-2/test-matrix.md` 的人工验收结果。
- 需要 CI artifact 时，记录 run id、commit SHA、artifact 名称与 SHA256。

## 新 CLI / TUI 功能的测试要求

- 新增或修改 CLI 端、TUI（后续功能）时，必须同时提供并在真实 Shell 中执行测试：
  - 本机：Windows PowerShell 5.1、PowerShell 7（pwsh）；
  - 远程：Bash、Zsh（`ssh guest_homeserver`）。
- 测试应优先写成可重复执行的小脚本（放在 `scripts/` 或临时目录），覆盖新增参数/交互路径、错误路径与清理路径。
- 每个 Shell 的结果独立记录，不能用一种 Shell 的结果替代另一种。
- 测试脚本与证据必须满足本文件“测试证据要求”；测试后清理临时配置根、脚本、二进制与远端临时目录。

## 数据同步与单一数据库（强制）

- **CLI、TUI、GUI 必须操作同一个 SQLite 数据库**：`<config-root>/aliases.db`。禁止各入口维护独立数据库、影子库或只读副本。
- 三个入口统一通过 `AppPaths::discover` 解析配置根；`--config-dir` 与 `ALIASMGR_CONFIG_DIR` 对 CLI/TUI/GUI 必须语义一致，指向同一根目录。
- 新增字段、表或迁移只能落在共享的 `aliasmgr-core` model / migration runner 中；禁止在某个前端单独建表或单独持久化业务数据。
- 写操作后其他入口必须重新读取数据库获得最新状态；GUI 刷新、CLI 重新执行、TUI 同此。任何缓存都必须可失效，且不得作为事实来源。
- 功能测试必须覆盖跨入口同步：CLI 写入后 GUI/TUI 可见；GUI/TUI 写入后 CLI 可见；同一配置根下只能存在一个 `aliases.db`。
- SQLite 并发继续使用现有锁/事务/忙等待机制；不得用“各自写自己的文件”来规避锁。
