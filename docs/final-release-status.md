# Alias Manager 最终发布状态

## 当前结论

当前分支已完成 Phase B 自动化验证和 Phase C 人工验收材料准备，但尚未满足发布条件。Task 21-2 人工结果尚未返回，因此不能标记人工案例为 `PASS`，也不能创建 tag、Release、签名或发布。

## 自动化工作流

| 工作流 | Run ID | 结果 | 范围 |
|---|---:|---|---|
| Fast CI | 32093176327 | PASS | Linux/Windows workspace、lint、GUI npm ci/cache、前端 test/typecheck/build |
| Integration | 32093828288 | PASS | Bash、Zsh、oh-my-zsh contract、PowerShell 5.1/7、lock、CLI、benchmark、隔离 |
| Release dry-run | 32093979406 | PASS | packaging dry-run、unsigned source archive、SHA256SUMS、sanitized manifest、security scan |

## 测试与产物

- 自动化测试清单：`docs/ci-test-inventory.md`
- Task 21-2 测试矩阵：`docs/task21-2/test-matrix.md`
- 未签名源代码 artifact：`alias-manager-unsigned-source`
- 产物内容：source archive、`SHA256SUMS`、sanitized `manifest.json`
- artifact 保留期：7 天
- 未执行自动签名、发布或 GitHub Release 创建。

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
