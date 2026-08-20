# 01-cli：CLI artifact 冒烟测试入口

本目录包含 CLI artifact 冒烟测试手册。这是整个验收流程的第一步，必须在执行 Shell、GUI 或安装包测试之前完成。

## 任务清单

开始执行前先打开：[task-list.md](task-list.md)。

清单中的脚本验证和人工验证必须分别完成。脚本结果只同步到对应脚本项目；不能用 CI 或 CLI 脚本结果代替真实用户配置、升级、回滚或安装包人工验收。

## 本目录包含的手册

| 手册文件 | 适用平台 | 说明 |
|---|---|---|
| [artifact-smoke.md](artifact-smoke.md) | 全平台 | artifact 下载、校验和清单说明 |
| [linux-cli.md](linux-cli.md) | Linux | Linux 终端 CLI 冒烟步骤 |
| [windows-cli.md](windows-cli.md) | Windows | Windows 终端 CLI 冒烟步骤 |

## 关于 CLI artifact 冒烟与 Shell 脚本的区别

- **CLI artifact 冒烟**（本目录）：验证 artifact 本身可以运行，版本正确，基础 CRUD 可用。这是判断 artifact 是否适合继续测试的门槛检查。
- **Shell 脚本验证**（`02-shell/` 目录）：在特定 Shell 环境中验证 loader 安装、RC 文件修改、幂等性、多 Shell 独立性等 Shell 集成行为。

两者相互独立，不能互相替代。

## CI 说明

CI 运行确定性的单元测试和集成测试，但无法代替在用户真实机器上运行的 artifact 冒烟测试。用户必须下载候选 artifact，亲自在目标平台上执行以下步骤，并记录结果。不得用 CI PASS 填写人工案例。
