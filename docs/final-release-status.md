# Alias Manager 最终发布状态

## 当前结论（2026-11-01）

文档与脚本收尾分支 `agent/ws7-docs` 已完成本地验证，但尚未推送，也没有触发任何 CI run。因此：

- 本地验证通过：Rust workspace 172 个测试、GUI Tauri crate 14 个测试、GUI 前端 223 个测试（含 `task20.test.ts`）及 typecheck/build。
- `cargo fmt --all -- --check`、GUI crate fmt check、`cargo clippy -D warnings` 均已在本地通过。
- 当前分支没有任何 CI 证据；不得声称 CI 成功。
- Task 21-2 人工验收仍待执行，不得把人工案例标记为 `PASS`。
- 未创建 tag、Release、签名或发布。

## 本地验证基线（当前分支）

| 检查 | 命令 | 结果 |
|---|---|---|
| Rust workspace | `cargo test --workspace --no-fail-fast`（隔离 `ALIASMGR_CONFIG_DIR`） | 172 passed / 0 failed |
| CLI 构建 | `cargo build -p aliasmgr-cli` | PASS |
| GUI Tauri crate | `cargo test --manifest-path crates/aliasmgr-gui/src-tauri/Cargo.toml`（先构建 `ui/dist`） | 14 passed / 0 failed |
| GUI 前端测试 | `npm test` | 223 passed / 0 failed |
| GUI 前端类型检查 | `npm run typecheck` | PASS |
| GUI 前端构建 | `npm run build` | PASS |
| 格式与 lint | `cargo fmt --all -- --check` / GUI crate `cargo fmt -- --check` / `cargo clippy --workspace --all-targets -- -D warnings` | PASS（本地） |

本机运行前需设置 `PATH="/d/mingw64/bin:/d/Program Files/nodejs:$PATH"` 和隔离的 `ALIASMGR_CONFIG_DIR`；详细命令见 `docs/testing.md`。

## CI 状态

| 工作流 | Run ID | 结果 | 说明 |
|---|---:|---|---|
| 当前分支 `agent/ws7-docs` | — | 未触发 | 分支尚未推送；无 CI 证据 |
| Fast CI（历史） | 32245518564 | PASS | commit ed28bac；Linux/Windows workspace、lint、GUI npm、前端 test/typecheck/build |
| Integration（历史） | 32245524584 | PASS | commit ed28bac；Bash、Zsh、oh-my-zsh contract、PowerShell 5.1/7、lock、CLI、benchmark、隔离 |
| Fast CI（更早） | 32093176327 | PASS | 上一基线 |
| Integration（更早） | 32093828288 | PASS | 上一基线 |
| Release dry-run（更早） | 32093979406 | PASS | packaging dry-run、unsigned source archive、SHA256SUMS、sanitized manifest、security scan |
| Prerelease workflow | — | 待执行 | `workflow_dispatch` only；需 `create_prerelease=true` + `confirmation=CREATE-PRERELEASE` |

历史 run 只对其对应 commit 生效，不代表当前分支。当前分支已合并 main（`52e0635`），但尚未推送，也没有任何 CI run；因此 CI 门禁、`gui-rust-windows` job 和 prerelease 打包都还没有在本分支上执行过。

## 测试与产物

- 自动化测试清单：`docs/ci-test-inventory.md`
- Task 21-2 测试矩阵：`docs/task21-2/test-matrix.md`（Actual/Result/Evidence 仍为空）
- 未签名源代码 artifact：`alias-manager-unsigned-source`（历史 release dry-run `32093979406`）
- 历史产物内容：source archive、`SHA256SUMS`、sanitized `manifest.json`
- artifact 保留期：7 天（release）/ 14 天（prerelease bundle）
- 当前分支未执行自动签名、发布或 GitHub Release 创建，也未产生新的打包 artifact。

### Prerelease 产物说明

`prerelease.yml` 定义了以下产物（`workflow_dispatch` 手动触发，不自动触发）：

| 产物 | 状态 |
|---|---|
| `aliasmgr-linux-x86_64-<version>` | CLI 二进制（未签名）— 工作流定义存在，当前分支未运行 |
| `aliasmgr-windows-x86_64-<version>.exe` | CLI 二进制（未签名）— 工作流定义存在，当前分支未运行 |
| `SHA256SUMS` | 所有成功构建 artifact 的 SHA256 校验和 |
| `manifest.json` | 清洁 manifest（`signed:false`、`published:false`、`release_created:false`），包含 GUI 构建状态 |
| `docs/task21-2/` | 中文验收手册 bundle |
| `scripts/verify-*.sh` / `scripts/verify-*.ps1` | 终端验证脚本 |
| `aliasmgr-linux-x86_64-<version>.AppImage` | Linux GUI AppImage — 仅在 Tauri 构建成功时存在 |
| `aliasmgr-linux-x86_64-<version>.deb` | Linux deb 安装包 — 仅在 Tauri 构建成功时存在 |
| `aliasmgr-windows-x86_64-<version>.msi` | Windows MSI 安装包 — 仅在 Tauri 构建成功时存在 |
| `aliasmgr-windows-x86_64-<version>-setup.exe` | Windows NSIS 安装程序 — 仅在 Tauri 构建成功时存在 |

GUI artifact 是否可用必须由一次成功的 remote CI run 证明。合并后的 `prerelease.yml` 现在 (a) 在 `crates/aliasmgr-gui/src-tauri` 目录内运行 `cargo tauri build`，(b) 通过生成在 workspace 根目录的 `tauri-prerelease.json` 文件传入 `{"bundle":{"active":true}}`（不再使用内联 JSON），(c) 仍然没有被本分支的任何 CI run 验证过。当前没有这样的运行证据，不能声明 GUI 安装包可用。

## 人工验收状态

以下中文手册已准备完成：

- `docs/task21-2/linux-clean-machine.md`
- `docs/task21-2/windows-powershell51.md`
- `docs/task21-2/windows-powershell7.md`
- `docs/task21-2/bash-zsh-user-config.md`
- `docs/task21-2/gui-workflow.md`
- `docs/task21-2/package-install-lifecycle.md`

目前 `test-matrix.md` 的 Actual、Result、Evidence 仍为空。必须由人工执行后填写，并为 FAIL/BLOCKED/EXPECTED-LIMITATION 案例填写中文反馈模板。脚本结果仍留在 `test/*-last-result` 作为历史记录；2026-11-01 修复 Windows 验证脚本后本地重跑得到 11 PASS + 2 EXPECTED-LIMITATION，但该结果尚未归档为正式人工验收证据。

## 环境阻塞与预期限制

- Windows ACL、Profile、OneDrive、BOM/CRLF、ExecutionPolicy、Group Policy、ConstrainedLanguage：需要真实 Windows 用户环境。
- oh-my-zsh 用户插件后置覆盖顺序：CI 只验证 contract，不替代真实用户插件环境。
- GUI 真实窗口、视觉布局、键盘焦点和可访问性：前端 CI 不替代人工窗口验收。
- MSI、Linux package hooks、安装/升级/回滚：当前没有可验收安装包输入。
- 语法检查是 best-effort：解释器不在 `PATH` 时跳过；PowerShell 解析器诊断语言依赖宿主 UI culture。
- import 预览使用只读 SQLite 句柄，可能留下 `-wal`/`-shm`，且不能建模所有插入时冲突。
- revision 当前由 alias/tombstone 行派生；未来若接入 tombstone 清理，需要额外保证单调性。
- 完整 SQLite transaction binding、真实 crash injection、live Shell fingerprint capture：仍是实现/发布审查边界。
- Fish/POSIX、import scanner、plugin API：Tasks 24–26，明确为 post-MVP/out-of-scope。

## 发布决策

发布条件尚未满足。必须先完成并审阅 Task 21-2 人工报告，处理或明确分类所有 FAIL/BLOCKED/EXPECTED-LIMITATION 案例，并让当前分支通过 CI 门禁、获得用户明确批准。当前没有创建 tag、Release、签名、合并或发布动作。
