# 03-gui：GUI 验收入口

本目录包含 Windows 和 Linux 的 GUI artifact 验收手册。GUI 测试必须在 CLI 和 Shell 测试完成之后进行。

---

## 前提条件

在开始 GUI 测试之前，确认：

1. CLI artifact 冒烟测试（`01-cli/`）已完成。
2. 至少一个 Shell 集成测试（`02-shell/`）已完成。
3. GUI artifact 已下载并通过 SHA256 校验（见 `01-cli/artifact-smoke.md`）。

---

## 本目录包含的手册

| 手册文件 | 适用平台 |
|---|---|
| [windows-gui.md](windows-gui.md) | Windows GUI |
| [linux-gui.md](linux-gui.md) | Linux GUI |

---

## 证据规则

每个 GUI 案例必须提供截图作为证据。提交前请确认：

- 截图中的路径、用户名、机器名必须脱敏（用 `<USER>`、`<MACHINE>` 等占位符替代）。
- 不要截取包含完整配置文件内容、API Key 或 Token 的界面。
- 截图文件路径记录在测试矩阵的 Evidence 列。

---

## CI 与 GUI 验收的边界

CI 前端测试验证了组件渲染和数据流，但**不能替代**以下真实 GUI 验收：

- 真实窗口启动和初始化行为
- 视觉布局和可访问性
- 用户交互反馈（鼠标悬停、焦点状态、键盘导航）
- 真实 runtime 持久化（设置保存后重启 GUI 的恢复行为）
- 错误弹窗和确认对话框的实际展示

所有 GUI 案例必须通过人工点击验收，不得用 CI 结果填写。
