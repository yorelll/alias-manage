# 04-package：安装包生命周期验收入口

本目录包含 Windows 和 Linux 的安装包生命周期验收手册。

---

## 重要前提

**只有在真实安装包 artifact 存在时才执行本目录中的测试。**

如果以下 artifact 不存在，不要执行对应测试，而是在矩阵中填写 `NOT-APPLICABLE` 或 `BLOCKED`：

| Artifact 类型 | 适用平台 | 对应手册 |
|---|---|---|
| `.msi` 或 `.exe` 安装器 | Windows | [windows-installer.md](windows-installer.md) |
| `.AppImage` | Linux | [linux-package.md](linux-package.md) |
| `.deb` 包 | Linux (Debian/Ubuntu) | [linux-package.md](linux-package.md) |
| `.rpm` 包 | Linux (Fedora/RHEL) | [linux-package.md](linux-package.md) |

---

## 执行顺序

安装包测试是最后一步，应在以下完成之后进行：

1. CLI 冒烟测试（`01-cli/`）
2. Shell 集成测试（`02-shell/`）
3. GUI 验收测试（`03-gui/`）

---

## 缺少 artifact 的处理

如果没有对应平台的安装包 artifact，在测试矩阵中记录：

| 情况 | 结果值 | 说明 |
|---|---|---|
| 安装包尚未生成（prerelease 阶段） | `NOT-APPLICABLE` | 写明"安装包 artifact 在此 prerelease 阶段未提供" |
| 安装包生成失败（CI 环境阻塞） | `BLOCKED` | 写明 CI 失败原因和阻塞 job 名称 |
| 安装包类型不适用于当前平台 | `NOT-APPLICABLE` | 写明"MSI 不适用于 Linux 平台" 等 |

绝不要将 unsigned source archive（如 `.tar.gz` 或 `.zip`）误认为安装包。
