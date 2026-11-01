# Task 21-2 CLI/Shell Task Lists Design

**状态：** 已确认方案 A

## 目标

在 `docs/task21-2/01-cli/` 和 `docs/task21-2/02-shell/` 下增加独立的中文任务清单，让用户可以按平台逐项勾选验证状态，并直接跳转到现有详细手册。清单同时区分脚本可验证项目和必须人工验证项目。

## 文件设计

新增：

- `docs/task21-2/01-cli/task-list.md`
- `docs/task21-2/02-shell/task-list.md`

修改：

- `docs/task21-2/01-cli/README.md`：增加任务清单入口。
- `docs/task21-2/02-shell/README.md`：增加任务清单入口。

现有 `test-matrix.md`、`feedback-template.md` 和详细平台手册继续作为结果与操作的事实来源，不重复搬运完整步骤。

## 清单结构

每个任务项包含：

- Markdown checkbox；
- 测试 ID；
- 简要验证内容；
- 验证类型：`脚本验证` 或 `人工验证`；
- 详细手册链接；
- 当前结果；
- 证据路径或待填写说明。

示例：

```markdown
- [x] L-001：Linux CLI 启动和版本
  - 类型：脚本验证
  - 当前结果：PASS
  - 详细步骤：[linux-cli.md](linux-cli.md)
  - 证据：`test/aliasmgr-last-result/verification-summary.json`

- [ ] L-003：Python/JAR/切换目录目标
  - 类型：人工验证
  - 当前结果：待执行
  - 详细步骤：[linux-cli.md](linux-cli.md)
  - 证据：执行后填写 `test-matrix.md`
```

## 勾选规则

- `[x]` 表示该任务已由对应证据完成。
- 脚本验证项只有在结果为 `PASS` 时勾选。
- `PASS_WITH_EXPECTED_LIMITATIONS` 只表示脚本必需检查没有失败，不等于所有人工边界完成。
- `EXPECTED-LIMITATION`、`BLOCKED`、`NOT-APPLICABLE` 保留实际结果，但不自动勾选为完成。
- 人工项只能由用户执行后勾选，并填写 Actual、Result、Evidence 和脱敏确认。
- 不从 CI 或脚本结果推断真实 Profile、当前 Shell 会话、GUI 窗口或视觉验收的人工 PASS。
- 所有结果值严格使用 `PASS | FAIL | BLOCKED | EXPECTED-LIMITATION | NOT-APPLICABLE`。

## 当前结果同步

根据 `test/` 中当前摘要：

- Linux CLI：脚本项 PASS，全部 33 个脚本案例可勾选。
- Linux Bash/Zsh：脚本 PASS 项可勾选；生成文件缺失、login chain、symlink、oh-my-zsh、当前会话等限制项保持未勾选并标记 `EXPECTED-LIMITATION` 或人工待执行。
- Windows CLI：脚本 PASS 项可勾选；tag 参数兼容性和 uninstall dry-run 限制项保持未勾选并记录原因。
- PowerShell 5.1/7：脚本 PASS 项可勾选；Profile/OneDrive、BOM/CRLF、native argv、内置别名、ExecutionPolicy/LanguageMode、loader 卸载等人工边界保持未勾选。

清单中的证据路径指向对应的脱敏摘要；实际反馈仍通过 `test-matrix.md` 和 `feedback-template.md` 提交。

## 范围边界

本次只增加导航和当前结果清单，不修改验证脚本行为，不修改结果枚举，不填写 GUI 或真实用户配置的人工 PASS，也不执行任何发布动作。
