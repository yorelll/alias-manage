# 限行（执行目录限制）

“限行”让一个别名只在特定目录内（白名单）或不在特定目录内（黑名单）执行。限制保存在别名记录中，并作为守卫代码写入生成的 Shell 包装函数，因此约束随生成文件生效，不依赖 `aliasmgr` 进程在运行时介入。

## 语义

- `whitelist`（白名单）：当前目录必须等于某个配置目录，或位于其子目录中，否则拒绝执行。
- `blacklist`（黑名单）：当前目录等于或位于任意配置目录之下时拒绝执行；其余目录放行。
- 目录匹配按路径组件边界进行：`/home/user` 不会匹配 `/home/user2`，但会匹配 `/home/user/project`。
- 目录在运行时规范化：当前目录和每个配置目录都会解析为物理路径（Bash/Zsh 使用 `pwd -P`，PowerShell 使用 `GetFullPath`）。符号链接因此按真实目标比较。
- 根目录 `/`（Windows 上为盘符根，如 `C:\`）匹配该根下的所有目录。
- 配置目录在运行时不存在时会被跳过，不会匹配。一个只包含失效目录的白名单会拒绝所有执行；黑名单中的失效目录不会阻止任何执行。
- 拒绝时向 stderr 输出一行，并以非零状态返回：
  - 白名单：`aliasmgr: refusing to run '<name>' in '<cwd>': current directory is not in the whitelist`
  - 黑名单：`aliasmgr: refusing to run '<name>' in '<cwd>': current directory is in the blacklist`

示例：

```bash
aliasmgr add rma --exec rm --arg -rf --restrict whitelist --restrict-path "$HOME/projects" --shell bash
```

生成的 `rma` 只允许在 `$HOME/projects` 及其子目录中运行：

```
aliasmgr: refusing to run 'rma' in '/tmp': current directory is not in the whitelist
```

黑名单示例：

```bash
aliasmgr add clean --exec rm --arg -rf --restrict blacklist --restrict-path / --restrict-path "$HOME"
```

`clean` 可以在任何位置运行，但 `/` 和 `$HOME` 之下的任意位置除外。

## CLI

`add` 与 `update` 都支持：

- `--restrict whitelist|blacklist`
- `--restrict-path <PATH>`（可重复；保存前会去除首尾空白）
- `--clear-restriction`（与上述两个参数互斥）

规则：

- `add`：`--restrict` 必须至少搭配一个 `--restrict-path`；只有 `--restrict-path` 而没有 `--restrict` 是配置错误。`--clear-restriction` 单独使用不会创建限制。
- `update`：
  - `--restrict` 单独使用只改变模式并保留已有目录；若记录原本没有限制则报错。
  - `--restrict-path` 单独使用替换目录列表并保留已有模式；若记录原本没有限制则报错。
  - `--clear-restriction` 删除限制。
  - 未提供的参数保持原值。

校验规则：目录列表至少一项，每项去除空白后非空、为绝对路径、不含 NUL，最多 64 项，每项最多 4096 个字符。校验失败按无效别名记录处理（配置错误，退出码 1）。

## GUI

别名向导中的“限行”复选框启用限制编辑器：

- 选择白名单或黑名单模式；
- 输入绝对路径后按 Enter 或点击“添加目录”加入列表；
- 已添加目录显示为可移除的条目；路径可以包含空格；
- 保存前会去除首尾空白、去重并校验绝对路径，但不要求目录当前存在。

列表详情行显示一个小徽章（例如“限行：白名单”）和配置目录。

## 限制与安全边界

限行是**防护栏，不是沙箱**：

- 只检查调用的当前工作目录。命令本身仍可 `cd` 到其他目录，或访问任意绝对路径。
- 直接按路径调用生成脚本、绕过 Shell 函数，或修改生成文件，都可绕过守卫。
- 符号链接会被规范化，因此通过符号链接进入受限目录同样会被识别；但检查与执行之间当前目录仍可能变化（例如命令内部或并发 Shell 操作）。
- 它不能替代文件权限、容器、虚拟机或操作系统级访问控制。
- 生成代码仍然只使用结构化参数和单引号字面量，不使用 `eval`、`Invoke-Expression` 或整行字符串求值。
