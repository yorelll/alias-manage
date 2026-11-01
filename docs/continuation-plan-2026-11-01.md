# 续作计划（2026-11-01）

本计划以 `docs/HANDOFF-2026-11-01.md` §8 为输入，按“先止血、再修契约、再补核心、最后补验证与文档”排序。执行方式：主 agent 规划与验收，子 agent A 实现，子 agent B 独立 review，A 修复直至 B 无异议；并行任务使用 git worktree，最后由主 agent 合并并复核。

## 基线（本次接手实测，历史记录）

- HEAD `684a752`（在交接文档 `961d70e` 之后补交了 `Cargo.lock` 与 `.gitignore`）。
- `cargo check --workspace --all-targets`：PASS。
- `cargo test --workspace --no-fail-fast`：113 pass / 1 fail（`aliasmgr-tests --test powershell` 并行隔离缺陷，§4.8）。
- `cargo fmt --all -- --check`：FAIL（37 文件）。
- `cargo clippy --workspace --all-targets -- -D warnings`：FAIL（3 处 `possible_missing_else`，均为 `shells/common.rs`）。
- GUI 前端 `npm test`：PASS（`task20.test.ts` 未纳入）。

## 工作流与验收

| 编号 | 内容 | 对应交接文档 | 验收 |
|---|---|---|---|
| WS1 | GUI Rust 层：语法损坏、依赖、workspace exclude、DTO 蛇形序列化、`created_at` 不可变导致 checksum 崩溃 | §4.1/§4.2/§7.6 | `rustc` 解析无语法错误；`cargo metadata` 成功；回归测试通过；`cargo check --manifest-path` 结果如实记录 |
| WS2 | CLI 契约：`update` 全字段且不破坏数据、`--env` 生效、`reload --shell`、NotFound→4、ConfirmationRequired→14、`import --yes`、`--quiet` | §4.3/§4.6 | 集成测试 + 真实二进制端到端输出 |
| WS3 | 配置目录统一（`AppPaths`）、Windows 默认目录、卸载移除 loader、`sync` revision 递增 | §4.7/§4.6/§4.13 | 只设 `ALIASMGR_CONFIG_DIR` 时生成文件落同一目录；卸载后 RC 无标记块 |
| WS4 | 核心：`reliability_error` 接线、备份轮转接线、PS5 BOM、PowerShell 测试隔离 | §4.13/§4.12/§4.8 | 单元/集成测试；`xxd` 验证 BOM；并行跑 powershell 测试通过 |
| WS5 | GUI 前端接线：列表/搜索/标签/向导/删除/设置/卸载面板 | §4.4 | `npm test`/typecheck/build；命令参数与 Rust 侧签名一致 |
| WS6 | CI 与门槛：去掉 fmt 前置写盘、clippy 修复、GUI Rust 编译检查、`task20.test.ts` 修复并纳入 | §4.5/§4.9/§4.11 | 本地 `fmt --check`、`clippy -D warnings` 通过 |
| WS7 | 文档与验收材料：修正过时说明、退出码/CLI 契约、误标完成项、Windows 验证脚本 bug | §5.5/§8.5 | 文档与实现一致；脚本不再产生假的 EXPECTED-LIMITATION |

## 执行状态（滚动更新）

- WS1 已合并：GUI Rust 语法损坏、依赖、workspace exclude、DTO 词表、`created_at` 不可变 checksum 修复；GUI crate `cargo check` 与 10 个单元测试通过。
- WS2 已合并：`update` 全字段非破坏、`--env`、`--arg` 连字符、`reload --shell`、NotFound=4、ConfirmationRequired=14、`import --yes` 原子落盘与敏感变量过滤、`--quiet`、`uninstall --yes`。
- 新增 `crates/aliasmgr-gui/src-tauri/icons/` 与 Tauri crate `Cargo.lock`，使 GUI 可编译。
- 合并后主分支测试：133 pass / 1 fail（唯一失败为本机未安装 PowerShell 7 的环境性发现测试，WS4b 将改为可跳过）。

- WS4b 已合并：文件系统可靠性接线、数据库备份轮转、导出脱敏、PowerShell 大小写折叠预览、PowerShell 测试隔离与 PS7 环境跳过。合并后工作区测试 147 pass / 0 fail。
- WS3/WS4a/WS5 已通过复审并进入最终修正/合并队列。
- WS6（CI 与门槛）仍在进行：`docs/ci-workflow.md` 由该工作流维护，本分支未复跑 `cargo fmt --check` / `cargo clippy -D warnings` 门禁。
- WS7 已完成本地收尾（分支 `agent/ws7-docs`，未推送、未触发 CI）：
  - 已更新 `plan.md`、`README.md`、`docs/testing.md`，把“本地无 Rust 工具链”改为本地预检 + CI 门禁，并保留 CI 边界。
  - 已按实现重写 `docs/cli-commands.md`，更新 `docs/exit-codes.md`、`docs/limitations.md`、`docs/release-checklist.md`、`docs/final-release-status.md`、`docs/ci-test-inventory.md` 与 `docs/task21-2/01-cli/task-list.md` 的过时陈述。
  - `scripts/verify-cli-windows.ps1` 的 W-008 已改为隔离配置目录下的 retain/purge 检查，W-006 注释说明 `--arg --oneline` 已支持。
  - 本地复验：workspace 172 tests pass；GUI Tauri crate 14 tests pass；GUI 前端 215 tests + typecheck/build pass；Windows CLI 脚本 11 PASS + 2 EXPECTED-LIMITATION，W-008 PASS，W-006 PASS。
  - `cargo fmt` / `cargo clippy` 与所有 CI/人工/打包项仍未完成；不得声称当前分支 CI 通过。

## 合并顺序

1. WS1 + WS2 并行（worktree）→ review → 修复 → 合并。
2. WS3 + WS4 + WS5 并行（worktree）→ review → 修复 → 合并。
3. WS6（依赖全部代码合并后，fmt 必须最后做一次全量格式化）。
4. WS7 文档收尾。
5. 主 agent 全量复跑：`fmt --check`、`clippy -D warnings`、`cargo test --workspace`、GUI `npm test/typecheck/build`、关键 CLI 端到端。

## 不做的范围（沿用 `plan.md` Task 24–26 与交接文档 §4.14）

Fish/POSIX 适配器、现有 Shell 配置导入扫描、WSL/CMD/Nushell/插件 API、别名递归解析、自动修改父 Shell、安装包签名与正式发布、高级 Shell 模式。人工验收（`docs/task21-2/test-matrix.md`）仍需真人在干净机器执行，本计划只修脚本与材料，不伪造 PASS。
