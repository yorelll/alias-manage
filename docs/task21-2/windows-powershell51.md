# Task 21-2 Windows PowerShell 5.1 人工验收

必须在 Windows PowerShell 5.1 下独立执行，不能用 `pwsh` 代替。使用临时 `USERPROFILE`、`LOCALAPPDATA`、`APPDATA` 和显式 Profile 覆盖。不得复用 PowerShell 7 结果。

每个案例记录：操作系统/架构、准确的 `$PSVersionTable.PSVersion`、ExecutionPolicy 各作用域、artifact SHA256、脱敏 Profile/配置路径、实际结果、结果枚举、证据、复现说明、敏感数据脱敏确认。

## PS51-001：干净启动和版本

1. 在临时位置安装或解压候选 artifact。
2. 使用隔离 Profile/配置目录启动 Windows PowerShell 5.1。
3. 记录应用和 Shell 版本。

预期：应用启动；版本正确；无关 Profile 未被修改。

## PS51-002：完整生命周期

1. 使用临时目标创建原生、Python、PowerShell 脚本和切换目录别名。
2. 修改目标、参数、描述和标签。
3. 执行列表、搜索、模糊搜索、排序、limit、标签筛选、同步、reload、禁用、改名和删除。
4. 新建隔离 PS 5.1 会话后重复关键检查。

预期：状态持久化；reload 只移除托管残留；错误保留草稿和配置。

## PS51-003：Profile 与 OneDrive

1. 记录 `$PROFILE.CurrentUserAllHosts`，但不要上传完整路径中的用户名。
2. 测试临时 Profile 覆盖路径。
3. 如果 Documents 被 OneDrive 重定向，使用实际解析路径重复测试。

预期：使用解析后的 Profile；覆盖路径优先；真实无关 Profile 不被修改。

## PS51-004：BOM/CRLF

1. 准备带 BOM 和 CRLF 的临时 Profile。
2. 安装和移除标记 loader。
3. 只记录字节编码和行尾摘要。

预期：原 Profile 编码和行尾保留；PS 5.1 生成文件遵守文档化 BOM 策略。

## PS51-005：内置别名与保留名

1. 使用临时定义测试 `ls`、`cp`、`gc` 抢占。
2. reload 前后检查 `Get-Command`，只记录类型和来源摘要。
3. 测试 ReadOnly/Constant 名称并记录错误分类。

预期：可移除冲突被抢占；保留定义返回 `NameReserved`；Constant 定义不被强制删除。

## PS51-006：ExecutionPolicy/语言模式

1. 记录测试用户各作用域的有效策略。
2. 仅在批准的临时策略环境测试 Restricted 和 AllSigned。
3. 如果存在 Group Policy 或 ConstrainedLanguage，记录状态，不绕过策略。

预期：Doctor 提供准确指引；不自动修改策略；不使用 `-ExecutionPolicy Bypass`；不支持状态标为 `EXPECTED-LIMITATION` 或 `BLOCKED`。

## PS51-007：原生 argv

1. 使用参数 dumper 测试空字符串、空格、引号、反斜杠、CJK、通配符和尾部反斜杠。
2. 测试中间 `{{args}}` 与固定参数。
3. 逐项记录 argv 边界；已知 PS 5.1 宿主重构限制标为 `EXPECTED-LIMITATION`。

## PS51-008：ACL/目标保护

1. 测试安全临时目标和其他用户可写路径（如果环境允许）。
2. 验证不安全目标的警告/阻止。
3. 执行 purge 卸载并确认目标字节未改变。

预期：未经验证的 ACL 不获得安全授权；目标保持完整。

## PS51-009：loader/卸载

1. 连续安装标记 loader 两次，比较脱敏 Profile hash/marker 数量。
2. 分别测试 retain 和 purge。
3. 确认无关 Profile 内容和目标文件保留。

预期：标记操作幂等且安全。

## PS51-010：MSI/安装包 hook

1. 如果提供 MSI/安装包 artifact，在临时 Windows 账户安装和卸载。
2. 记录 hook 是否非交互，以及是否只调用文档化清理边界。
3. 如果没有 artifact，标记 `NOT-APPLICABLE` 或 `BLOCKED` 并写明缺少的输入。

预期：hook 不静默修改无关配置，也不删除目标文件。
