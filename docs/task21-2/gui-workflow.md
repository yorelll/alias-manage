# Task 21-2 GUI 工作流人工验收

必须在 Linux 和 Windows 分别运行候选 GUI artifact。CI 前端测试不能替代真实窗口、视觉、可访问性和 runtime persistence 验收。记录系统/架构、GUI 版本、artifact SHA256、脱敏配置路径、显示环境、实际结果、结果枚举、证据、复现说明和脱敏确认。

## GUI-001/GUI-002：启动、状态和导航

1. 启动 artifact。
2. 确认状态抽屉显示版本、检测到的 Shell 和配置目录。
3. 依次进入 Aliases、Sync、Doctor、Settings。

预期：窗口启动；状态准确；导航不崩溃且不丢草稿。

## GUI-003/GUI-004：表格、搜索和分面

1. 创建带描述、目标类型、Shell、enabled 状态和标签的别名。
2. 检查字段、长描述 tooltip、稳定 tag chip 和状态。
3. 测试文本、模糊、字段、limit、单标签、多标签 AND 和清除。

预期：字段正确渲染；tooltip 显示完整描述；筛选由 core 执行；清除恢复全部。

## GUI-005/GUI-006：向导和校验

1. 打开 Add/Edit 向导。
2. 测试基础/高级、结构化参数、中间 `{{args}}`、cwd、环境、Shell、标签、预览、Save、Cancel。
3. 测试非法、保留、精确冲突和 PowerShell case-fold 冲突名称。

预期：非法保存拒绝；草稿保留；不渲染秘密；仅确认后保存。

## GUI-007：Doctor

1. 准备缺失生成文件、过期状态和安全/人工 findings。
2. 展开 per-Shell 详情。
3. 使用安全操作并复制人工 reload 指引。

预期：严重度、详情和操作准确；人工边界明确；不自动绕过 Profile/ExecutionPolicy。

## GUI-008：导入

1. 选择临时 JSON/TOML。
2. 预览并验证没有持久化。
3. 确认并验证 accepted records 持久化。
4. 测试 malformed、unsupported、sensitive、relative-path 和 conflict 输入。

预期：warnings/skipped/unsupported 明确保留；确认失败保留报告和草稿。

## GUI-009：设置

1. 修改备份/日志保留、默认 Shell、配置目录和 relative-path 设置。
2. 取消并验证草稿重置。
3. 保存并重启 GUI。
4. 仅在临时目录测试只读/无效配置路径。

预期：设置持久化；失败保留草稿并说明边界；不未经选择修改真实配置。

## GUI-010：卸载

1. 创建引用临时目标的别名。
2. 分别打开 retain/purge 选项。
3. 检查风险摘要和二次确认。
4. 执行一个模式并检查无关内容和目标字节。

预期：retain/purge 明确；必须二次确认；目标和无关内容保留。

## GUI-011：覆盖定义

1. 通过批准流程创建可恢复和不可恢复 override record。
2. 打开详情并只复制脱敏 metadata/恢复文本。

预期：recoverability 准确显示；复制不执行目标、不暴露私有内容。

## GUI-012：可访问性和视觉布局

1. 仅用键盘导航。
2. 检查焦点顺序、可见焦点、标签、错误提示和公告。
3. 调整到最小和宽窗口，检查表格、modal、tooltip 和关键操作。

预期：无焦点陷阱、关键按钮裁切、不可读对比度或布局破坏；截图中的路径和内容必须脱敏。
