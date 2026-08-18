# Task 21-2 人工辅助验收

这是发布边界的人工验收轨道。不得从 CI 推断人工结果。请在干净机器、专用用户或临时配置目录上执行对应平台手册，并返回填写完成的 `test-matrix.md`，以及每个 `FAIL`、`BLOCKED`、`EXPECTED-LIMITATION` 案例对应的 `feedback-template.md` 条目。

## 结果值

每个案例必须且只能使用一个结果：

- `PASS`：观察到预期结果。
- `FAIL`：发现实现或发布缺陷。
- `BLOCKED`：环境阻止执行；必须写明准确阻塞原因。
- `EXPECTED-LIMITATION`：行为符合已文档化限制。
- `NOT-APPLICABLE`：案例不适用于当前平台；必须解释原因。

## 测试前安全要求

- 尽量使用专用临时用户、Profile、RC、配置目录和目标文件。
- 不要在报告中粘贴完整 Profile、环境变量转储、Token、密码、API Key 或私有目标内容。
- 对截图、日志和路径中的用户名、机器名、临时目录进行脱敏。
- 记录操作系统、架构、Shell/版本、应用或安装包版本、commit 或 artifact SHA256，以及脱敏后的配置目录。
- 如果出现意外删除、Profile 损坏、秘密泄露或数据丢失，立即停止，标记 `FAIL` 并保留证据。

## 案例执行规则

平台手册只是操作说明，不是测试结果。人工执行前，`Actual`、`Result`、`Evidence` 必须保持空白。CI、源代码检查、前端构建或自动化测试可以证明自动化边界，但不能为人工案例产生 `PASS`。

## 反馈提交

请返回填写完成的 `test-matrix.md`，并为每个 `FAIL`、`BLOCKED`、`EXPECTED-LIMITATION` 案例复制填写 `feedback-template.md`。在审阅报告之前，不会据此修改发布复选框。

人工验收不自动授权合并、打 tag、发布、签名或推送。

## 测试组

- Linux：`linux-clean-machine.md`
- Windows PowerShell 5.1：`windows-powershell51.md`
- Windows PowerShell 7：`windows-powershell7.md`
- Bash/Zsh 用户配置：`bash-zsh-user-config.md`
- GUI：`gui-workflow.md`
- 安装包/安装生命周期：`package-install-lifecycle.md`

PowerShell 5.1 与 7 必须独立重复相同场景；一个版本的结果不能替代另一个版本。
