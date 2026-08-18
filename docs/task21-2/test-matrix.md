# Task 21-2 人工验收测试矩阵

每个实际执行的案例都必须填写 `Actual`、`Result` 和 `Evidence`。不得根据 CI 结果填写人工结果。每个案例的结果必须严格使用 `PASS | FAIL | BLOCKED | EXPECTED-LIMITATION | NOT-APPLICABLE`。

## 元数据字段说明

每次执行时，在矩阵上方填写本次执行的元数据：

- **执行时间：** （留空，由执行者填写）
- **执行者：** （留空，由执行者填写）
- **Artifact 版本：** （留空，由执行者填写）
- **Artifact SHA256：** （留空，由执行者填写）
- **脚本摘要路径：** （留空，由执行者填写，指向 result/verification-summary.json）
- **完整日志路径：** （留空，由执行者填写，指向 result/verification.log）

---

## CLI 案例（01-cli/）

| ID | 平台/Shell | 功能 | 预期 | 实际 | 结果 | Artifact SHA256 | 脚本摘要路径 | 日志路径 | 截图路径 |
|---|---|---|---|---|---|---|---|---|---|
| L-001 | Linux | 干净安装/启动/版本 | 应用启动并报告版本 | | | | | | |
| L-002 | Linux | 添加原生别名 | 别名持久化并出现在列表 | | | | | | |
| L-003 | Linux | Python/JAR/切换目录 | 每种目标类型校验并正确渲染 | | | | | | |
| L-004 | Linux | 空格/引号/CJK/反斜杠/通配符 | 参数保留准确边界 | | | | | | |
| L-005 | Linux | `{{args}}` 末尾/中间/非法/重复 | 合法位置生效；非法形式拒绝 | | | | | | |
| L-006 | Linux | 工作目录/环境变量 | 目标在配置上下文运行且不泄露秘密 | | | | | | |
| L-007 | Linux | 列表/搜索/模糊/limit | 结果和排序符合请求 | | | | | | |
| L-008 | Linux | 标签分面单选/多选/清除 | 多标签使用 AND；清除恢复全部 | | | | | | |
| L-017 | Linux | JSON/TOML 导入 | 预览、警告、确认、持久化正确 | | | | | | |
| L-018 | Linux | retain/purge 卸载 | 选定清理执行；目标保留 | | | | | | |
| L-019 | Linux | 升级/回滚 | 配置和生成状态正确保留或恢复 | | | | | | |

---

## Shell 案例（02-shell/）

| ID | 平台/Shell | 功能 | 预期 | 实际 | 结果 | Artifact SHA256 | 脚本摘要路径 | 日志路径 | 截图路径 |
|---|---|---|---|---|---|---|---|---|---|
| L-009 | Linux Bash | loader/reload | 标记 loader 加载生成别名 | | | | | | |
| L-010 | Linux Zsh | loader/reload | 标记 loader 加载生成别名 | | | | | | |
| L-011 | Linux Bash/Zsh | login/非交互 | 观察到文档化交互行为和 login 链 | | | | | | |
| L-012 | Linux Zsh | oh-my-zsh 顺序 | 报告 loader 后覆盖且不静默修改 | | | | | | |
| L-013 | Linux | RC symlink/行尾 | symlink 保留；行尾保留 | | | | | | |
| L-014 | Linux | 手工修改生成文件 | 产生备份并要求用户决策 | | | | | | |
| L-015 | Linux | 禁用/改名/删除/tombstone | reload 移除托管残留但不删除用户定义 | | | | | | |
| L-016 | Linux | 同步失败/恢复 | 恢复先前状态且 per-Shell 状态准确 | | | | | | |
| PS51-001 | Windows PS 5.1 | 干净安装/启动/版本 | 应用和 PS 5.1 Profile 正常 | | | | | | |
| PS51-002 | Windows PS 5.1 | 完整生命周期 | 增删改查、搜索、标签、同步、reload、删除正常 | | | | | | |
| PS51-003 | Windows PS 5.1 | Profile/OneDrive | Profile 解析正确且与真实配置隔离 | | | | | | |
| PS51-004 | Windows PS 5.1 | BOM/CRLF | 编码和行尾保留 | | | | | | |
| PS51-005 | Windows PS 5.1 | 内置别名 | `ls`/`cp`/`gc` 抢占和保留名行为正确 | | | | | | |
| PS51-006 | Windows PS 5.1 | ExecutionPolicy | Restricted/AllSigned/Group Policy/ConstrainedLanguage 指引准确 | | | | | | |
| PS51-007 | Windows PS 5.1 | 原生 argv | 参数边界矩阵符合预期 | | | | | | |
| PS51-008 | Windows PS 5.1 | ACL/目标保护 | 不安全路径警告/阻止且目标保留 | | | | | | |
| PS51-009 | Windows PS 5.1 | loader/卸载 | 标记修改幂等且安全 | | | | | | |
| PS7-001 | Windows PS 7 | 干净安装/启动/版本 | 应用和 PS 7 Profile 正常 | | | | | | |
| PS7-002 | Windows PS 7 | 完整生命周期 | 独立完成增删改查、搜索、标签、同步、reload、删除 | | | | | | |
| PS7-003 | Windows PS 7 | Profile/OneDrive | Profile 解析正确且与真实配置隔离 | | | | | | |
| PS7-004 | Windows PS 7 | BOM/CRLF | 编码和行尾保留 | | | | | | |
| PS7-005 | Windows PS 7 | 内置别名 | `ls`/`cp`/`gc` 抢占和保留名行为正确 | | | | | | |
| PS7-006 | Windows PS 7 | ExecutionPolicy | Restricted/AllSigned/Group Policy/ConstrainedLanguage 指引准确 | | | | | | |
| PS7-007 | Windows PS 7 | 原生 argv | 参数边界矩阵符合预期 | | | | | | |
| PS7-008 | Windows PS 7 | ACL/目标保护 | 不安全路径警告/阻止且目标保留 | | | | | | |
| PS7-009 | Windows PS 7 | loader/卸载 | 标记修改幂等且安全 | | | | | | |

---

## GUI 案例（03-gui/）

| ID | 平台/Shell | 功能 | 预期 | 实际 | 结果 | Artifact SHA256 | 脚本摘要路径 | 日志路径 | 截图路径 |
|---|---|---|---|---|---|---|---|---|---|
| GUI-001 | Linux/Windows | 启动/版本/状态 | 窗口启动，状态抽屉显示版本/Shell/配置 | | | | | | |
| GUI-002 | Linux/Windows | 侧边栏 | Aliases/Sync/Doctor/Settings 导航正常 | | | | | | |
| GUI-003 | Linux/Windows | 表格/描述/标签 | 字段、tooltip、chip、状态正确渲染 | | | | | | |
| GUI-004 | Linux/Windows | 搜索/分面 | 文本/模糊/limit/标签 AND/清除正常 | | | | | | |
| GUI-005 | Linux/Windows | 向导 | 基础/高级/预览/保存/取消正常 | | | | | | |
| GUI-006 | Linux/Windows | 校验/冲突 | 非法/保留/精确/case-fold 错误保留草稿 | | | | | | |
| GUI-007 | Linux/Windows | Doctor | 摘要、详情、操作和人工边界准确 | | | | | | |
| GUI-008 | Linux/Windows | 导入 | 预览/确认/失败保留正常 | | | | | | |
| GUI-009 | Linux/Windows | 设置 | 草稿/保存/失败/只读配置路径正常 | | | | | | |
| GUI-010 | Linux/Windows | 卸载 | retain/purge/风险/二次确认正常 | | | | | | |
| GUI-011 | Linux/Windows | 覆盖定义 | 复制/可恢复状态正常且不执行目标 | | | | | | |
| GUI-012 | Linux/Windows | UX/可访问性 | 键盘/焦点/缩放/视觉布局可接受 | | | | | | |

---

## 安装包案例（04-package/）

| ID | 平台/Shell | 功能 | 预期 | 实际 | 结果 | Artifact SHA256 | 脚本摘要路径 | 日志路径 | 截图路径 |
|---|---|---|---|---|---|---|---|---|---|
| PS51-010 | Windows | MSI/安装包 hook | 安装器/卸载器遵循 retain/purge 边界 | | | | | | |
| PKG-001 | Linux/Windows | 干净安装包安装 | 安装不修改无关配置 | | | | | | |
| PKG-002 | Linux/Windows | 升级/回滚 | 升级回滚保留预期状态 | | | | | | |
| PKG-003 | Linux/Windows | hook/卸载 | postrm/MSI 非交互且安全 | | | | | | |
| PKG-004 | Linux/Windows | 非托管内容 | 无关 Profile/文件保持不变 | | | | | | |

---

`Actual`、`Result`、`Evidence` 只能由人工执行后填写。结果枚举必须严格使用 `PASS | FAIL | BLOCKED | EXPECTED-LIMITATION | NOT-APPLICABLE`。
