# Task 21-2 Windows PowerShell 7 人工验收

必须在 `pwsh` 下独立执行，不能复用 PowerShell 5.1 结果。记录 PS 7 准确版本、操作系统/架构、策略和语言模式、artifact SHA256、脱敏 Profile/配置路径、实际结果、结果枚举、证据、复现说明和脱敏确认。

## PS7-001/PS7-002：干净启动和生命周期

1. 使用隔离 PS 7 Profile/配置目录启动候选应用。
2. 独立完成增删改查、搜索、模糊搜索、limit、标签、同步、reload、禁用、改名和删除。
3. 新建 PS 7 会话后重复持久化检查。

预期：PS 7 状态独立持久化；reload 行为正确；不声称子进程能更新父会话。

## PS7-003：Profile 与 OneDrive

1. 记录 `$PROFILE.CurrentUserAllHosts` 和来源摘要，不暴露用户名。
2. 测试显式 Profile 覆盖。
3. 如果 Documents 被 OneDrive 重定向，重复实际路径解析测试。

预期：PS 7 与 PS 5.1 路径独立；覆盖路径优先；真实无关 Profile 不被修改。

## PS7-004：编码和行尾

1. 准备 BOM/no-BOM、CRLF/LF 的临时 Profile 变体。
2. 安装/移除 loader 并记录字节摘要。
3. 单独检查 PS 7 生成文件编码。

预期：Profile 字节保持；PS 7 生成文件遵循文档化无 BOM 策略。

## PS7-005：抢占和保留名

1. 使用临时定义测试 `ls`、`cp`、`gc`。
2. 仅记录 `Get-Command` 类型/来源摘要。
3. 测试 ReadOnly/Constant 名称并记录拒绝结果。

预期：可移除 alias/function 被清理；保留名被拒绝；用户定义不被静默删除。

## PS7-006：策略和语言模式

1. 记录策略作用域和 `$ExecutionContext.SessionState.LanguageMode`。
2. 仅在隔离策略环境测试 Restricted、AllSigned、Group Policy、ConstrainedLanguage。
3. 不使用绕过开关或自动改变策略。

预期：指引准确；限制或阻塞状态明确记录。

## PS7-007：原生 argv

1. 使用 dumper 执行空、空格、引号、反斜杠、CJK、通配符、尾部反斜杠参数矩阵。
2. 测试中间 `{{args}}` 和固定参数。
3. 如果存在多个 PS 7 版本，比较宿主参数传递差异；逐版本记录。

## PS7-008：ACL 和目标保护

1. 测试安全和不安全临时目标位置。
2. 验证警告/阻止行为。
3. purge 后确认引用目标字节未改变。

## PS7-009：loader/卸载

1. 连续安装标记 loader 两次。
2. 分别测试 retain 和 purge。
3. 验证无关 Profile 内容、生成文件和目标保护。

预期：标记修改幂等；不删除非托管内容。
