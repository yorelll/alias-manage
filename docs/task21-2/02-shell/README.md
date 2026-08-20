# 02-shell：Shell 集成验证入口

本目录包含各 Shell 环境的集成验证手册。在执行本目录中的任何手册之前，请先完成 `01-cli/` 目录中的 CLI artifact 冒烟测试。

---

## 任务清单

开始执行前先打开：[task-list.md](task-list.md)。

Bash、Zsh、PowerShell 5.1 和 PowerShell 7 必须分别执行。脚本通过不代表真实 login chain、Profile/OneDrive、symlink、当前会话或插件顺序已经人工通过。结果提交请使用 [`../test-matrix.md`](../test-matrix.md) 和 [`../feedback-template.md`](../feedback-template.md)。

## 支持的 Shell 和手册

| 手册文件 | 平台 | Shell | 支持状态 |
|---|---|---|---|
| [linux-bash.md](linux-bash.md) | Linux | Bash | 完全支持 |
| [linux-zsh.md](linux-zsh.md) | Linux | Zsh | 完全支持 |
| [windows-powershell51.md](windows-powershell51.md) | Windows | PowerShell 5.1 | 完全支持 |
| [windows-powershell7.md](windows-powershell7.md) | Windows | PowerShell 7 | 完全支持 |
| [windows-git-bash.md](windows-git-bash.md) | Windows | Git Bash | 探索性（有限制，见手册说明） |

---

## 执行顺序

各 Shell 的验证可以并行进行，但必须满足以下条件：

1. PowerShell 5.1 和 PowerShell 7 **必须独立执行**，一个 Shell 的结果不能替代另一个。
2. Bash 和 Zsh **必须独立执行**，即使两者都在 Linux 上。
3. 每个 Shell 手册必须在对应的 Shell 进程中执行，不能跨 Shell 替代。

---

## 手动跟进案例

以下案例无法自动化，必须手动执行并记录：

- 真实 login 链检查（`login` Bash / Zsh）：自动化测试只覆盖交互非登录 Shell。
- oh-my-zsh 顺序检查：需要真实 oh-my-zsh 环境。
- 当前会话 reload：自动化脚本无法在父进程中验证别名注入效果。
- 可信 symlink RC 保留：需要实际符号链接环境。
- 真实 OneDrive Profile 重定向：需要真实 OneDrive 配置。

---

## 安全提示

- 始终使用专用临时目录或专用测试用户。
- 不要修改主账户的真实 RC 文件（`.bashrc`、`.zshrc`、`$PROFILE`）。
- 测试完成后，清理所有临时 RC 文件和配置目录。
- 报告中不要粘贴完整的 RC 文件内容、环境变量转储或 Token。
