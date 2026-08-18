# Linux GUI 验收手册

涵盖案例：GUI-001 至 GUI-012（Linux 平台部分）

在开始之前，请确认已完成 Linux CLI 和 Bash/Zsh Shell 冒烟测试，并准备好 GUI artifact 及其 SHA256 校验。

---

## 准备工作

1. 确认有可用的图形显示环境（本地桌面、VNC 或 X11 转发）。
2. 确认 artifact 为 Linux GUI 二进制或 AppImage：

   ```bash
   ls /tmp/aliasmgr-smoke/
   # 预期看到 aliasmgr-gui 或 aliasmgr.AppImage
   ```

3. 如果是 AppImage，添加执行权限：

   ```bash
   chmod +x /tmp/aliasmgr-smoke/aliasmgr-gui.AppImage
   ```

4. 确认使用**专用测试用户**，不要在主账户上测试。

---

## GUI-001/GUI-002：启动、状态和导航

### 点击路径

1. 打开文件管理器或终端，双击或运行 GUI artifact：

   ```bash
   /tmp/aliasmgr-smoke/aliasmgr-gui
   # 或 AppImage：
   /tmp/aliasmgr-smoke/aliasmgr-gui.AppImage
   ```

2. 等待窗口出现（不超过 10 秒）。
3. **截图点 1**：截取启动后的初始界面（脱敏路径，使用截图工具如 `gnome-screenshot` 或 `scrot`）。
4. 点击右上角或右侧的**状态图标**（或"状态抽屉"按钮）。
5. 确认状态抽屉显示：
   - 应用版本号
   - 检测到的 Shell 列表（如 `bash 5.1`、`zsh 5.9`）
   - 配置目录路径（记录脱敏路径）
6. **截图点 2**：截取状态抽屉内容（脱敏路径）。
7. 依次点击左侧导航栏：
   - 点击 **Aliases**
   - 点击 **Sync**
   - 点击 **Doctor**
   - 点击 **Settings**
8. **截图点 3**：截取任意一个导航页面。

### 预期 UI 状态

- 窗口正常启动；状态信息准确；四个导航页面均可进入，不崩溃，不丢草稿。

### 错误处理

- 如果 AppImage 运行失败（缺少 FUSE 或 libfuse2），记录错误，标记 `BLOCKED` 并说明环境原因。
- 如果显示环境不支持（Wayland/X11 问题），记录，标记 `BLOCKED`。

---

## GUI-003/GUI-004：表格、搜索和分面

### 准备

用 CLI 添加测试别名：

```bash
aliasmgr add --name gui-test-native --target "echo hello" --shell bash --description "GUI 表格测试" --tag "gui" --tag "smoke"
aliasmgr add --name gui-test-long-desc --target "echo test" --shell bash --description "这是一段很长的描述文字，用于测试 tooltip 和截断行为，不应该因为过长而导致布局破坏" --tag "gui"
aliasmgr add --name gui-test-disabled --target "echo disabled" --shell bash --tag "smoke"
aliasmgr disable gui-test-disabled
```

### 点击路径

1. 点击左侧导航 → **Aliases**。
2. 确认 `gui-test-native` 在表格中显示，字段正确。
3. **截图点 4**：截取别名列表表格。
4. 鼠标悬停在 `gui-test-long-desc` 的描述列，确认 tooltip 显示完整描述。
5. **截图点 5**：截取 tooltip 状态。
6. 在搜索框输入 `gui`，确认过滤结果正确。
7. 清空搜索框，确认全部恢复。
8. 点击 `smoke` 标签筛选，确认过滤结果正确。
9. 同时选中 `gui` 和 `smoke` 标签，确认为 AND 关系。
10. 点击清除筛选，确认全部恢复。
11. **截图点 6**：截取标签筛选状态。

### 预期 UI 状态

- 字段正确渲染；tooltip 显示完整描述；AND 筛选正确；清除恢复全部。

---

## GUI-005/GUI-006：向导和校验

### 点击路径（基础向导）

1. 点击左侧导航 → **Aliases** → 点击 **Add** 按钮。
2. 在 **Name** 输入框输入 `gui-wizard-test`。
3. 在 **Target** 输入框输入 `echo wizard-ok`。
4. 在 **Shell** 下拉菜单选择 `bash`。
5. 在 **Description** 输入框输入 `向导测试`。
6. 点击 **Advanced** 标签 → 在 **CWD** 输入 `/tmp`（临时目录）。
7. 点击 **Parameters** → 添加固定参数 `--dry-run`。
8. 在 **Tags** 添加 `gui`。
9. 点击 **Preview**，确认预览不执行命令。
10. **截图点 7**：截取向导预览状态。
11. 点击 **Save**，确认别名出现在列表。

### 点击路径（校验错误测试）

1. 再次点击 **Add**。
2. 输入已有名称 `gui-wizard-test`，确认错误提示"名称已存在"。
3. **截图点 8**：截取错误提示。
4. 修改名称为 `ls`（保留名或冲突名），确认错误提示。
5. 点击 **Cancel**，确认草稿清除。

### 预期 UI 状态

- 非法名称被拒绝；草稿在取消后清除；不渲染私有内容。

---

## GUI-007：Doctor

### 准备

> **警告：** 必须在专用测试用户账户下操作，绝不在主账户上执行本步骤。专用测试用户的配置目录不含生产数据，删除其中的生成文件不会影响主账户。

制造轻微异常（删除专用测试用户的生成文件）以触发 Doctor 发现：

```bash
# 仅在专用测试用户账户下执行；主账户请勿执行此操作
rm -f ~/.config/aliasmgr/generated/aliases.bash
```

如果无法使用专用测试用户，可改为在隔离临时配置目录中制造异常：

```bash
rm -f $SMOKE_CFG/generated/aliases.bash
```

### 点击路径

1. 点击左侧导航 → **Doctor**。
2. 等待扫描完成，查看 finding 摘要。
3. **截图点 9**：截取 Doctor 摘要页面。
4. 展开某个 finding 的详情，查看 per-Shell 详情。
5. 如有安全操作，点击执行；如有手动操作提示，复制提示文本。
6. **截图点 10**：截取展开详情。

### 预期 UI 状态

- 摘要准确；人工边界明确；不自动绕过 Profile 限制。

---

## GUI-008：导入

### 准备

```bash
cat > /tmp/gui-import-test.json << 'EOF'
[
  {"name": "gui-import-linux", "target": "echo imported", "shell": "bash"},
  {"name": "INVALID NAME", "target": "bad", "shell": "bash"}
]
EOF
```

### 点击路径

1. 点击左侧导航 → **Aliases** → 点击 **Import** 按钮。
2. 在文件对话框中选择 `/tmp/gui-import-test.json`。
3. 确认预览显示 `gui-import-linux` 将导入，`INVALID NAME` 显示跳过原因。
4. 点击 **Cancel**，确认列表无变化（预览只读）。
5. 再次导入，点击 **Confirm**。
6. 确认 `gui-import-linux` 出现在列表中，`INVALID NAME` 不在列表中。
7. **截图点 11**：截取导入结果。

---

## GUI-009：设置

### 点击路径

1. 点击左侧导航 → **Settings**。
2. 修改"默认 Shell"设置为另一个值。
3. 点击 **Cancel**，确认值恢复原状。
4. 再次修改，点击 **Save**。
5. 关闭 GUI，重新启动，确认设置保留。
6. **截图点 12**：截取设置页面（脱敏路径）。
7. 测试只读路径：在配置目录字段输入 `/root/invalid`，确认有错误提示。

---

## GUI-010：卸载

### 准备

```bash
touch /tmp/gui-target.sh
aliasmgr add --name gui-uninstall-test --target "bash /tmp/gui-target.sh" --shell bash
```

### 点击路径

1. 在 GUI 中打开 Settings → 找到"卸载"入口，点击进入。
2. 选择 **Retain** 模式。
3. 确认风险摘要显示。
4. 点击二次确认。
5. **截图点 13**：截取卸载结果。
6. 验证目标文件未被删除：

   ```bash
   test -f /tmp/gui-target.sh && echo "目标保留: OK"
   ```

---

## GUI-011：覆盖定义

### 点击路径

1. 在 Bash RC 中添加同名用户 alias（临时 RC 中），触发覆盖状态。
2. 在 GUI 别名列表中找到显示覆盖状态的别名，点击详情。
3. 查看 override metadata（可恢复性）。
4. 点击"复制恢复文本"，粘贴到文本编辑器检查内容（确认只有脱敏 metadata）。
5. **截图点 14**：截取覆盖详情（脱敏）。

---

## GUI-012：可访问性和视觉布局

### 键盘导航

1. 移开鼠标，用 **Tab** 遍历所有可交互元素。
2. 用 **Enter/Space** 激活按钮，确认功能正常。
3. **截图点 15**：截取焦点状态（某个按钮或输入框高亮）。

### 视觉布局

1. 缩小窗口，检查按钮是否裁切，表格是否可滚动。
2. 拉宽到全屏，检查布局是否合理。
3. **截图点 16**：最小窗口状态。
4. **截图点 17**：最大化窗口状态。

### 预期 UI 状态

- 无焦点陷阱；关键按钮不裁切；布局不破坏；对比度可读。

---

## AppImage 特有注意事项

如果使用 AppImage：
- 如果 FUSE 不可用，尝试 `--appimage-extract-and-run` 选项：
  ```bash
  /tmp/aliasmgr-smoke/aliasmgr-gui.AppImage --appimage-extract-and-run
  ```
- 记录使用的启动方式和任何额外警告（标记 `EXPECTED-LIMITATION`）。

---

## 清理

```bash
rm -f /tmp/gui-import-test.json /tmp/gui-target.sh
```
