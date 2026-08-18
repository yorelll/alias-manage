# Windows GUI 验收手册

涵盖案例：GUI-001 至 GUI-012（Windows 平台部分）

在开始之前，请确认已完成 CLI 和 Shell 冒烟测试，并准备好 GUI artifact 及其 SHA256 校验。

---

## 准备工作

1. 打开文件管理器，找到已下载的 GUI artifact（`.exe` 格式或解压后的文件夹）。
2. 核对 SHA256（参见 [../01-cli/artifact-smoke.md](../01-cli/artifact-smoke.md)）。
3. 确认使用**专用测试用户**，不要在主账户上测试。
4. 关闭其他应用，避免干扰焦点测试。

---

## GUI-001/GUI-002：启动、状态和导航

### 点击路径

1. 双击 GUI artifact 启动应用。
2. 等待窗口出现（不超过 10 秒）。
3. **截图点 1**：截取启动后的初始界面（脱敏路径）。
4. 点击右上角或右侧的**状态图标**（或"状态抽屉"按钮）。
5. 确认状态抽屉显示：
   - 应用版本号
   - 检测到的 Shell 列表（如 `PowerShell 5.1`、`PowerShell 7`）
   - 配置目录路径（脱敏后记录）
6. **截图点 2**：截取状态抽屉内容（脱敏路径）。
7. 点击左侧导航栏 → **Aliases**。
8. 点击左侧导航栏 → **Sync**。
9. 点击左侧导航栏 → **Doctor**。
10. 点击左侧导航栏 → **Settings**。
11. **截图点 3**：截取导航全程中任意一个页面。

### 预期 UI 状态

- 窗口正常启动，无白屏或崩溃。
- 状态信息准确显示（版本和 Shell 数量合理）。
- 四个导航页面均可进入，不崩溃，不丢失草稿。

### 错误处理

- 如果窗口 10 秒内未出现：记录错误信息，标记 `FAIL`。
- 如果状态抽屉显示空白：记录，标记 `FAIL`。
- 如果导航到某页面后崩溃：记录崩溃信息，标记 `FAIL`。

---

## GUI-003/GUI-004：表格、搜索和分面

### 准备

先用 CLI 工具添加若干别名，以便测试表格显示：

```powershell
& aliasmgr add --name gui-test-native --target "Write-Output hello" --shell powershell --description "GUI 表格测试" --tag "gui" --tag "smoke"
& aliasmgr add --name gui-test-long-desc --target "Write-Output test" --shell powershell --description "这是一段很长的描述文字，用于测试 tooltip 和截断行为，不应该因为过长而导致布局破坏" --tag "gui"
& aliasmgr add --name gui-test-disabled --target "Write-Output disabled" --shell powershell --tag "smoke"
& aliasmgr disable gui-test-disabled
```

### 点击路径

1. 点击左侧导航 → **Aliases**。
2. 在表格中确认 `gui-test-native` 出现，字段（名称、目标、Shell、enabled 状态、标签）正确显示。
3. **截图点 4**：截取别名列表表格。
4. 鼠标悬停在 `gui-test-long-desc` 的描述列上，确认完整描述以 tooltip 形式显示。
5. **截图点 5**：截取 tooltip 显示状态。
6. 在搜索框输入 `gui`，确认过滤结果只包含含 `gui` 的别名。
7. 清空搜索框，确认全部别名恢复显示。
8. 点击标签筛选区域中的 `smoke`，确认只显示带 `smoke` 标签的别名。
9. 同时点击 `gui` 和 `smoke` 标签，确认为 AND 关系（只显示同时有两个标签的别名）。
10. 点击清除筛选按钮，确认全部别名恢复显示。
11. **截图点 6**：截取标签筛选状态。

### 预期 UI 状态

- 字段正确渲染；tooltip 显示完整描述；标签 AND 筛选正确；清除恢复全部。

---

## GUI-005/GUI-006：向导和校验

### 点击路径（基础向导）

1. 点击左侧导航 → **Aliases**。
2. 点击 **Add** 按钮（或"新增别名"按钮）。
3. 向导打开，在 **Name** 输入框输入 `gui-wizard-test`。
4. 在 **Target** 输入框输入 `Write-Output wizard-ok`。
5. 在 **Shell** 下拉菜单选择 `PowerShell`。
6. 在 **Description** 输入框输入 `向导测试`。
7. 点击 **Advanced** 标签（或"高级"选项卡）。
8. 在 **CWD** 输入框输入临时目录路径（脱敏）。
9. 点击 **Parameters** 区域，添加一个固定参数 `--dry-run`。
10. 在 **Tags** 输入框输入 `gui`，按 Enter 添加。
11. 点击 **Preview**（预览）按钮，确认渲染结果正确且不执行命令。
12. **截图点 7**：截取向导预览状态。
13. 点击 **Save**，确认别名出现在列表中。

### 点击路径（校验错误测试）

1. 再次点击 **Add** 按钮。
2. 在 **Name** 输入框输入 `gui-wizard-test`（与已有别名同名）。
3. 确认错误提示显示（如"名称已存在"）。
4. **截图点 8**：截取错误提示状态。
5. 修改名称为 `ls`（PowerShell 保留名）。
6. 确认 case-fold 冲突或保留名错误提示显示。
7. 点击 **Cancel**，确认草稿被清除且不保存。

### 预期 UI 状态

- 非法名称保存被拒绝；错误提示清晰；草稿在取消后清除；不渲染秘密内容。

---

## GUI-007：Doctor

### 准备

> **警告：** 必须在专用测试用户账户下操作，绝不在主账户上执行本步骤。专用测试用户的配置目录不含生产数据，删除其中的生成文件不会影响主账户。

制造一个轻微异常状态（删除专用测试用户的生成文件）以触发 Doctor 发现：

```powershell
# 仅在专用测试用户账户下执行；主账户请勿执行此操作
Remove-Item "$env:APPDATA\aliasmgr\generated\*.ps1" -Force -ErrorAction SilentlyContinue
```

如果无法使用专用测试用户，可改为在隔离临时配置目录中制造异常（将 `$PS51Cfg` 或 `$PS7Cfg` 替换为当前测试的临时目录变量）：

```powershell
Remove-Item "$PS51Cfg\generated\*.ps1" -Force -ErrorAction SilentlyContinue
```

### 点击路径

1. 点击左侧导航 → **Doctor**。
2. 等待 Doctor 扫描完成。
3. **截图点 9**：截取 Doctor 摘要页面。
4. 点击某个 finding 的详情箭头（或"展开"按钮），查看 per-Shell 详情。
5. 如果有"安全操作"建议，点击执行（限于文档化的安全自动操作）。
6. 如果有"手动操作"提示（如 reload 别名），复制提示文本，**不要**点击自动重启。
7. **截图点 10**：截取展开后的详情。

### 预期 UI 状态

- 严重度、详情和操作准确；人工操作边界明确标注；不自动绕过 Profile/ExecutionPolicy。

---

## GUI-008：导入

### 准备

准备临时 JSON 导入文件：

```powershell
@'
[
  {"name": "gui-import-test", "target": "Write-Output imported", "shell": "powershell"},
  {"name": "INVALID NAME WITH SPACES", "target": "bad", "shell": "powershell"}
]
'@ | Set-Content "$env:TEMP\gui-import-test.json" -Encoding UTF8
```

### 点击路径

1. 点击左侧导航 → **Aliases**，然后点击 **Import** 按钮（或导入图标）。
2. 在文件选择对话框中，选择 `gui-import-test.json`。
3. 预览页面显示后，确认：
   - `gui-import-test` 显示为"将要导入"。
   - `INVALID NAME WITH SPACES` 显示为"将跳过"或"无效"，附原因。
4. **不要**点击确认按钮。点击 **Cancel** 退出。
5. 确认列表中没有出现新导入的别名（预览只读验证）。
6. 再次导入，这次点击 **Confirm**（确认）。
7. 确认 `gui-import-test` 出现在列表中；`INVALID NAME WITH SPACES` 不在列表中。
8. **截图点 11**：截取导入结果。

### 预期 UI 状态

- 预览只读，确认前不持久化；无效记录明确显示跳过原因；确认后正确导入。

---

## GUI-009：设置

### 点击路径

1. 点击左侧导航 → **Settings**（设置）。
2. 找到"默认 Shell"设置，修改为另一个值（如从 PowerShell 7 改为 PowerShell 5.1）。
3. 点击 **Cancel**，确认设置未保存（值恢复原状）。
4. 再次修改并点击 **Save**。
5. 关闭 GUI，重新启动。
6. 进入 Settings，确认保存的设置保留。
7. **截图点 12**：截取设置页面（脱敏路径）。
8. 测试只读配置路径：在配置目录字段输入不存在或只读的路径，确认有错误提示，草稿保留。

### 预期 UI 状态

- 取消时草稿重置；保存后重启恢复；失败时保留草稿并说明边界；不未经选择修改真实配置。

---

## GUI-010：卸载

### 点击路径

1. 先创建引用临时目标文件的别名：

   ```powershell
   "# gui uninstall target" | Set-Content "$env:TEMP\gui-target.ps1" -Encoding UTF8
   & aliasmgr add --name gui-uninstall-test --target "pwsh -NoProfile -File $env:TEMP\gui-target.ps1" --shell powershell
   ```

2. 在 GUI 中点击左侧导航 → **Settings**（或卸载入口）。
3. 找到"卸载"按钮，点击进入卸载流程。
4. 选择 **Retain**（保留数据）模式。
5. 确认风险摘要显示（列出将要保留和将要清除的内容）。
6. 点击二次确认按钮。
7. **截图点 13**：截取卸载结果。
8. 验证临时目标文件未被删除：

   ```powershell
   Test-Path "$env:TEMP\gui-target.ps1"
   ```

### 预期 UI 状态

- retain/purge 模式明确区分；必须经过二次确认；目标文件和无关内容保留。

---

## GUI-011：覆盖定义

### 点击路径

1. 先在 Shell 中创建一个与应用别名同名的用户 alias（在临时 Profile 中）。
2. 在 GUI 中找到覆盖状态的别名，点击详情图标。
3. 查看 override metadata（显示可恢复性状态）。
4. 点击"复制恢复文本"按钮，粘贴到文本编辑器中。
5. 确认复制的内容只包含脱敏 metadata，不包含私有脚本内容。
6. **截图点 14**：截取覆盖详情页面（脱敏内容）。

### 预期 UI 状态

- recoverability 准确显示；复制不执行目标；不暴露私有内容。

---

## GUI-012：可访问性和视觉布局

### 点击路径（键盘导航）

1. 将鼠标移开，使用 **Tab** 键在 GUI 中导航。
2. 按 Tab 顺序遍历所有可交互元素，确认每个元素都有可见焦点样式。
3. 使用 **Enter** 或 **Space** 激活按钮，确认功能正常。
4. 尝试用键盘打开向导、输入名称并取消。
5. **截图点 15**：截取键盘焦点状态（高亮某个按钮或输入框）。

### 点击路径（视觉布局）

1. 将窗口缩小到最小允许尺寸，检查：
   - 关键按钮是否被裁切
   - 表格是否可滚动
   - Modal 对话框是否仍完整显示
2. 将窗口拉宽到全屏，检查布局是否合理（不过于分散）。
3. **截图点 16**：截取最小窗口状态。
4. **截图点 17**：截取最大化窗口状态。

### 预期 UI 状态

- 无焦点陷阱；关键按钮不被裁切；对比度和字体大小可读；布局不破坏。

---

## 保存结果摘要

完成所有 GUI 案例后，将截图文件路径记录到测试矩阵的 Evidence 列，并填写 Actual 和 Result。

**提交前检查：**

- [ ] 所有截图中的路径已脱敏（用 `<USER>` 替代用户名）
- [ ] 截图中无密码、Token 或 API Key 可见
- [ ] 每个案例的 Evidence 列已填写截图路径
