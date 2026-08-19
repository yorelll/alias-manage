# Alias Manager 最终发布状态

## 当前结论

当前分支已完成 Phase B 自动化验证和 Phase C 人工验收材料准备，但尚未满足发布条件。Task 21-2 人工结果尚未返回，因此不能标记人工案例为 `PASS`，也不能创建 tag、Release、签名或发布。

## 自动化工作流

| 工作流 | Run ID | 结果 | 范围 |
|---|---:|---|---|
| Fast CI | 32245518564 | PASS | Linux/Windows workspace、lint、GUI npm ci/cache、前端 test/typecheck/build（commit ed28bac） |
| Integration | 32245524584 | PASS | Bash、Zsh、oh-my-zsh contract、PowerShell 5.1/7、lock、CLI、benchmark、隔离（commit ed28bac） |
| Fast CI (prior) | 32093176327 | PASS | 上一基线 |
| Integration (prior) | 32093828288 | PASS | 上一基线 |
| Release dry-run | 32093979406 | PASS | packaging dry-run、unsigned source archive、SHA256SUMS、sanitized manifest、security scan |
| Prerelease workflow | — | 待执行 | `workflow_dispatch` only；门禁：Fast CI + Integration + Task 23；需 `create_prerelease=true` + `confirmation=CREATE-PRERELEASE` 才创建 prerelease |

## 测试与产物

- 自动化测试清单：`docs/ci-test-inventory.md`
- Task 21-2 测试矩阵：`docs/task21-2/test-matrix.md`
- 未签名源代码 artifact：`alias-manager-unsigned-source`（release dry-run `32093979406`）
- 产物内容：source archive、`SHA256SUMS`、sanitized `manifest.json`
- artifact 保留期：7 天（release）/ 14 天（prerelease bundle）
- 未执行自动签名、发布或 GitHub Release 创建。

### Prerelease 产物说明

`prerelease.yml` 产生以下产物（`workflow_dispatch` 手动触发，不自动触发）：

| 产物 | 状态 |
|---|---|
| `aliasmgr-linux-x86_64-<version>` | CLI 二进制（未签名）— 始终存在 |
| `aliasmgr-windows-x86_64-<version>.exe` | CLI 二进制（未签名）— 始终存在 |
| `SHA256SUMS` | 所有成功构建 artifact 的 SHA256 校验和 |
| `manifest.json` | 清洁 manifest（`signed:false`、`published:false`、`release_created:false`）— 包含 GUI 构建状态 |
| `docs/task21-2/` | 中文验收手册 bundle |
| `scripts/verify-*.sh` / `scripts/verify-*.ps1` | 终端验证脚本 |
| `aliasmgr-linux-x86_64-<version>.AppImage` | Linux GUI AppImage — 仅在 Tauri 构建成功时存在 |
| `aliasmgr-linux-x86_64-<version>.deb` | Linux deb 安装包 — 仅在 Tauri 构建成功时存在 |
| `aliasmgr-windows-x86_64-<version>.msi` | Windows MSI 安装包 — 仅在 Tauri 构建成功时存在 |
| `aliasmgr-windows-x86_64-<version>-setup.exe` | Windows NSIS 安装程序 — 仅在 Tauri 构建成功时存在 |

**关于 GUI artifact 的说明：**
- `prerelease.yml` 现在包含实际的 Tauri 构建 job：Linux 使用 `build_linux_gui`（ubuntu-24.04），Windows 使用 `build_windows_gui`（windows-latest）。
- 每个 job 会安装原生依赖（Linux：webkit2gtk 4.1/appindicator；Windows：检测 WebView2），运行 `cargo tauri build`，并收集产生的 AppImage/deb/MSI/NSIS 文件。
- 如果构建失败，job 在 manifest 中记录 `gui_build_status: environment-blocked`，并上传诊断 artifact，但不伪造 GUI 二进制。
- 只有当 remote CI 证明 Tauri 构建成功后，才能声明 GUI artifact 可用。当前状态为待 CI 验证。

**注意：** 不在实现阶段触发 prerelease 创建。prerelease URL 只在用户明确授权且创建工作流成功后更新。

## 人工验收状态

以下中文手册已准备完成：

- `docs/task21-2/linux-clean-machine.md`
- `docs/task21-2/windows-powershell51.md`
- `docs/task21-2/windows-powershell7.md`
- `docs/task21-2/bash-zsh-user-config.md`
- `docs/task21-2/gui-workflow.md`
- `docs/task21-2/package-install-lifecycle.md`

目前 `test-matrix.md` 的 Actual、Result、Evidence 仍为空。必须由人工执行后填写，并为 FAIL/BLOCKED/EXPECTED-LIMITATION 案例填写中文反馈模板。

## 环境阻塞与预期限制

- Windows ACL、Profile、OneDrive、BOM/CRLF、ExecutionPolicy、Group Policy、ConstrainedLanguage：需要真实 Windows 用户环境。
- oh-my-zsh 用户插件后置覆盖顺序：CI 只验证 contract，不替代真实用户插件环境。
- GUI 真实窗口、视觉布局、键盘焦点和可访问性：前端 CI 不替代人工窗口验收。
- MSI、Linux package hooks、安装/升级/回滚：当前没有可验收安装包输入。
- 完整 SQLite transaction binding、真实 crash injection、live Shell fingerprint capture：仍是实现/发布审查边界。
- Fish/POSIX、import scanner、plugin API：Tasks 24–26，明确为 post-MVP/out-of-scope。

## 发布决策

发布条件尚未满足。必须先完成并审阅 Task 21-2 人工报告，处理或明确分类所有 FAIL/BLOCKED/EXPECTED-LIMITATION 案例，并获得用户明确批准。当前没有创建 tag、Release、签名、合并或发布动作。
