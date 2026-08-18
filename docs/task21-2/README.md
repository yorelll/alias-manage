# Task 21-2 人工辅助验收

这是发布边界的人工验收轨道。不得从 CI 推断人工结果。请在干净机器、专用用户或临时配置目录上执行对应平台手册，并返回填写完成的 `test-matrix.md`，以及每个 `FAIL`、`BLOCKED`、`EXPECTED-LIMITATION` 案例对应的 `feedback-template.md` 条目。

---

## 执行顺序（必须严格遵守）

以下步骤必须按顺序完成。前一步骤完成前不得跳到下一步。

### 第一步：准备 artifact 和 checksum

在执行任何测试之前，先从 GitHub Actions 工件页面下载候选 artifact，并用 `SHA256SUMS` 文件校验其完整性。记录 artifact 名称、版本和 SHA256。

详见 [01-cli/artifact-smoke.md](01-cli/artifact-smoke.md)。

### 第二步：CLI artifact 冒烟测试

在干净机器或临时用户上运行 CLI artifact 冒烟测试，验证 artifact 可启动、报告正确版本、基础 CRUD 可用。

**说明：** CI 覆盖确定性单元/集成测试，但 artifact 冒烟测试必须在用户机器上执行——不能用 CI 结果替代。

- Linux CLI：[01-cli/linux-cli.md](01-cli/linux-cli.md)
- Windows CLI：[01-cli/windows-cli.md](01-cli/windows-cli.md)

### 第三步：Shell 脚本验证

在每个支持的 Shell 独立执行脚本验证。各 Shell 的结果相互独立，不能替代。

- Linux Bash：[02-shell/linux-bash.md](02-shell/linux-bash.md)
- Linux Zsh：[02-shell/linux-zsh.md](02-shell/linux-zsh.md)
- Windows PowerShell 5.1：[02-shell/windows-powershell51.md](02-shell/windows-powershell51.md)
- Windows PowerShell 7：[02-shell/windows-powershell7.md](02-shell/windows-powershell7.md)
- Windows Git Bash（探索性）：[02-shell/windows-git-bash.md](02-shell/windows-git-bash.md)

### 第四步：边缘/安全检查

完成上述 CLI 和 Shell 验证后，针对 argv 边界、ExecutionPolicy、ACL 目标保护等边缘案例进行专项检查。这些案例在对应的 Shell 手册中有单独章节。

### 第五步：真实用户配置测试

使用真实 RC/Profile（或隔离等效环境），测试 loader 安装、reload、幂等性和卸载。不要使用主机上的生产配置——始终使用专用临时目录。

### 第六步：Windows GUI 测试

在 Windows 机器上启动 GUI artifact，按点击路径手册逐项验收。

详见：[03-gui/windows-gui.md](03-gui/windows-gui.md)

### 第七步：Linux GUI 测试

在 Linux 机器（或带显示环境的 VM）上启动 GUI artifact，按手册逐项验收。

详见：[03-gui/linux-gui.md](03-gui/linux-gui.md)

### 第八步：安装包生命周期测试

只在有安装包 artifact（MSI / EXE / AppImage / .deb）时执行此步骤。若缺少 artifact，在矩阵中标记 `NOT-APPLICABLE` 或 `BLOCKED` 并说明原因。

- Windows 安装器：[04-package/windows-installer.md](04-package/windows-installer.md)
- Linux 安装包：[04-package/linux-package.md](04-package/linux-package.md)

---

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

---

## 原始手册（兼容入口）

以下文件是早期扁平结构的兼容保留入口，其内容已整合进上方有序分目录手册中。建议直接使用上方手册；如需快速查阅，可参考这些文件。

- 旧 Linux 综合手册：[linux-clean-machine.md](linux-clean-machine.md)
- 旧 Windows PS 5.1 手册：[windows-powershell51.md](windows-powershell51.md)
- 旧 Windows PS 7 手册：[windows-powershell7.md](windows-powershell7.md)
- 旧 Bash/Zsh 用户配置手册：[bash-zsh-user-config.md](bash-zsh-user-config.md)
- 旧 GUI 工作流手册：[gui-workflow.md](gui-workflow.md)
- 旧安装包生命周期手册：[package-install-lifecycle.md](package-install-lifecycle.md)
