# CLI 任务清单

## 当前脚本结果元数据

当前结果来源：`test/aliasmgr-last-result/verification-summary.json` 和 `test/aliasmgr-win-last-result/verification-summary.json`。
当前 CLI 脚本结果：Linux `PASS`（33/33）；Windows `PASS_WITH_EXPECTED_LIMITATIONS`（9 PASS，4 EXPECTED-LIMITATION）。

结果枚举：`PASS | FAIL | BLOCKED | EXPECTED-LIMITATION | NOT-APPLICABLE`

### 复选框规则

- `[x]` 表示该条目有对应 `PASS` 证据。`[ ]` 表示待执行。
- 脚本项目：只在摘要 JSON 中对应案例结果为 `PASS` 时才打勾。
- `EXPECTED-LIMITATION`、`BLOCKED`、`NOT-APPLICABLE` 保持未勾选，并在条目中显示确切结果和原因。
- 人工项目：只有在执行者在 `test-matrix.md`（或 `feedback-template.md`）中填写了 Actual、Result、Evidence 并确认脱敏之后，才能打勾。
- 禁止用 CI 或脚本结果代替真实用户配置、升级、回滚或安装包的人工验收。

---

## 准备和校验

在执行任何 CLI 测试之前，必须先完成 artifact 下载和 SHA256 校验。

详见：[artifact-smoke.md](artifact-smoke.md)

- [ ] 准备-001：从 GitHub Actions 下载候选 artifact
  - 类型：人工操作
  - 当前结果：待执行
  - 详细说明：[artifact-smoke.md — 第一步：找到候选 artifact](artifact-smoke.md)
  - 证据：执行后填写 `docs/task21-2/test-matrix.md`

- [ ] 准备-002：校验 artifact SHA256
  - 类型：人工操作
  - 当前结果：待执行
  - 详细说明：[artifact-smoke.md — 第二步：校验 SHA256](artifact-smoke.md)
  - 证据：执行后填写 `docs/task21-2/test-matrix.md`

- [ ] 准备-003：解压 artifact 并记录版本和路径信息
  - 类型：人工操作
  - 当前结果：待执行
  - 详细说明：[artifact-smoke.md — 第三步：解压 artifact](artifact-smoke.md)
  - 证据：执行后填写 `docs/task21-2/test-matrix.md`

---

## Linux CLI 脚本验证

本节所有条目均来自 `scripts/verify-cli-linux.sh` 的自动运行结果。
证据：`test/aliasmgr-last-result/verification-summary.json`

- [x] L-001：版本和启动
  - 类型：脚本验证
  - 当前结果：PASS
  - 详细说明：[linux-cli.md — L-001：干净安装/启动/版本](linux-cli.md)
  - 证据：`test/aliasmgr-last-result/verification-summary.json`

- [x] L-002：添加原生别名
  - 类型：脚本验证
  - 当前结果：PASS
  - 详细说明：[linux-cli.md — L-002：添加原生别名](linux-cli.md)
  - 证据：`test/aliasmgr-last-result/verification-summary.json`

- [x] L-002b：列表包含已添加的条目
  - 类型：脚本验证
  - 当前结果：PASS
  - 详细说明：[linux-cli.md — L-002：添加原生别名](linux-cli.md)
  - 证据：`test/aliasmgr-last-result/verification-summary.json`

- [x] L-003：通过名称获取别名
  - 类型：脚本验证
  - 当前结果：PASS
  - 详细说明：[linux-cli.md — L-002：添加原生别名](linux-cli.md)
  - 证据：`test/aliasmgr-last-result/verification-summary.json`

- [x] L-003b：禁用/启用循环
  - 类型：脚本验证
  - 当前结果：PASS
  - 详细说明：[linux-cli.md — L-002：添加原生别名](linux-cli.md)
  - 证据：`test/aliasmgr-last-result/verification-summary.json`

- [x] L-003c：重命名别名
  - 类型：脚本验证
  - 当前结果：PASS
  - 详细说明：[linux-cli.md — L-002：添加原生别名](linux-cli.md)
  - 证据：`test/aliasmgr-last-result/verification-summary.json`

- [x] L-003d：更新别名
  - 类型：脚本验证
  - 当前结果：PASS
  - 详细说明：[linux-cli.md — L-002：添加原生别名](linux-cli.md)
  - 证据：`test/aliasmgr-last-result/verification-summary.json`

- [x] L-004：argv 空格/引号/CJK/反斜杠/通配符
  - 类型：脚本验证
  - 当前结果：PASS
  - 详细说明：[linux-cli.md — L-004：argv 精确边界](linux-cli.md)
  - 证据：`test/aliasmgr-last-result/verification-summary.json`

- [x] L-004-ENV：工作目录/环境隔离标记
  - 类型：脚本验证
  - 当前结果：PASS
  - 详细说明：[linux-cli.md — L-006：工作目录/环境/标签](linux-cli.md)
  - 证据：`test/aliasmgr-last-result/verification-summary.json`

- [x] L-005：`{{args}}` 尾部位置合法
  - 类型：脚本验证
  - 当前结果：PASS
  - 详细说明：[linux-cli.md — L-005：`{{args}}` 矩阵](linux-cli.md)
  - 证据：`test/aliasmgr-last-result/verification-summary.json`

- [x] L-005b：重复 `{{args}}` 被拒绝
  - 类型：脚本验证
  - 当前结果：PASS
  - 详细说明：[linux-cli.md — L-005：`{{args}}` 矩阵](linux-cli.md)
  - 证据：`test/aliasmgr-last-result/verification-summary.json`

- [x] L-005c：`{{args}}` 与 `--no-pass-args` 同时使用被拒绝
  - 类型：脚本验证
  - 当前结果：PASS
  - 详细说明：[linux-cli.md — L-005：`{{args}}` 矩阵](linux-cli.md)
  - 证据：`test/aliasmgr-last-result/verification-summary.json`

- [x] L-006：同步生成 Shell 文件
  - 类型：脚本验证
  - 当前结果：PASS
  - 详细说明：[linux-cli.md — L-006：工作目录/环境/标签](linux-cli.md)
  - 证据：`test/aliasmgr-last-result/verification-summary.json`

- [x] L-007：list --limit 5
  - 类型：脚本验证
  - 当前结果：PASS
  - 详细说明：[linux-cli.md — L-007/L-008：搜索和标签分面](linux-cli.md)
  - 证据：`test/aliasmgr-last-result/verification-summary.json`

- [x] L-007b：按名称查找
  - 类型：脚本验证
  - 当前结果：PASS
  - 详细说明：[linux-cli.md — L-007/L-008：搜索和标签分面](linux-cli.md)
  - 证据：`test/aliasmgr-last-result/verification-summary.json`

- [x] L-007c：find --fuzzy
  - 类型：脚本验证
  - 当前结果：PASS
  - 详细说明：[linux-cli.md — L-007/L-008：搜索和标签分面](linux-cli.md)
  - 证据：`test/aliasmgr-last-result/verification-summary.json`

- [x] L-008：标签过滤单选
  - 类型：脚本验证
  - 当前结果：PASS
  - 详细说明：[linux-cli.md — L-007/L-008：搜索和标签分面](linux-cli.md)
  - 证据：`test/aliasmgr-last-result/verification-summary.json`

- [x] L-008b：标签过滤多选（AND）
  - 类型：脚本验证
  - 当前结果：PASS
  - 详细说明：[linux-cli.md — L-007/L-008：搜索和标签分面](linux-cli.md)
  - 证据：`test/aliasmgr-last-result/verification-summary.json`

- [x] L-009：shell 检测
  - 类型：脚本验证
  - 当前结果：PASS
  - 详细说明：[linux-cli.md](linux-cli.md)
  - 证据：`test/aliasmgr-last-result/verification-summary.json`

- [x] L-010：doctor 命令
  - 类型：脚本验证
  - 当前结果：PASS
  - 详细说明：[linux-cli.md](linux-cli.md)
  - 证据：`test/aliasmgr-last-result/verification-summary.json`

- [x] L-011：reload --print 显示生成路径
  - 类型：脚本验证
  - 当前结果：PASS
  - 详细说明：[linux-cli.md](linux-cli.md)
  - 证据：`test/aliasmgr-last-result/verification-summary.json`

- [x] L-012：导出 JSON 文件
  - 类型：脚本验证
  - 当前结果：PASS
  - 详细说明：[linux-cli.md — L-017：JSON/TOML 导入](linux-cli.md)
  - 证据：`test/aliasmgr-last-result/verification-summary.json`

- [x] L-012b：导出 TOML 文件
  - 类型：脚本验证
  - 当前结果：PASS
  - 详细说明：[linux-cli.md — L-017：JSON/TOML 导入](linux-cli.md)
  - 证据：`test/aliasmgr-last-result/verification-summary.json`

- [x] L-013：导入 JSON 文件
  - 类型：脚本验证
  - 当前结果：PASS
  - 详细说明：[linux-cli.md — L-017：JSON/TOML 导入](linux-cli.md)
  - 证据：`test/aliasmgr-last-result/verification-summary.json`

- [x] L-014：导入 TOML 文件
  - 类型：脚本验证
  - 当前结果：PASS
  - 详细说明：[linux-cli.md — L-017：JSON/TOML 导入](linux-cli.md)
  - 证据：`test/aliasmgr-last-result/verification-summary.json`

- [x] L-015：sync --dry-run 非破坏性
  - 类型：脚本验证
  - 当前结果：PASS
  - 详细说明：[linux-cli.md](linux-cli.md)
  - 证据：`test/aliasmgr-last-result/verification-summary.json`

- [x] L-016：非法别名名称被拒绝
  - 类型：脚本验证
  - 当前结果：PASS
  - 详细说明：[linux-cli.md](linux-cli.md)
  - 证据：`test/aliasmgr-last-result/verification-summary.json`

- [x] L-016b：以数字开头的名称被拒绝
  - 类型：脚本验证
  - 当前结果：PASS
  - 详细说明：[linux-cli.md](linux-cli.md)
  - 证据：`test/aliasmgr-last-result/verification-summary.json`

- [x] L-016c：空可执行文件被拒绝
  - 类型：脚本验证
  - 当前结果：PASS
  - 详细说明：[linux-cli.md](linux-cli.md)
  - 证据：`test/aliasmgr-last-result/verification-summary.json`

- [x] L-017：使用 --yes 删除别名
  - 类型：脚本验证
  - 当前结果：PASS
  - 详细说明：[linux-cli.md — L-018/L-019：卸载、升级和回滚](linux-cli.md)
  - 证据：`test/aliasmgr-last-result/verification-summary.json`

- [x] L-017b：删除后列表中不存在该别名
  - 类型：脚本验证
  - 当前结果：PASS
  - 详细说明：[linux-cli.md — L-018/L-019：卸载、升级和回滚](linux-cli.md)
  - 证据：`test/aliasmgr-last-result/verification-summary.json`

- [x] L-018：将 bash loader 安装到隔离的 RC 文件
  - 类型：脚本验证
  - 当前结果：PASS
  - 详细说明：[linux-cli.md — L-018/L-019：卸载、升级和回滚](linux-cli.md)
  - 证据：`test/aliasmgr-last-result/verification-summary.json`

- [x] L-019：loader 安装幂等性
  - 类型：脚本验证
  - 当前结果：PASS
  - 详细说明：[linux-cli.md — L-018/L-019：卸载、升级和回滚](linux-cli.md)
  - 证据：`test/aliasmgr-last-result/verification-summary.json`

---

## Windows CLI 脚本验证

本节所有条目均来自 `scripts/verify-cli-windows.ps1` 的自动运行结果。
证据：`test/aliasmgr-win-last-result/verification-summary.json`

- [x] W-001：版本和启动
  - 类型：脚本验证
  - 当前结果：PASS
  - 详细说明：[windows-cli.md — PS51-001 / PS7-001（CLI 部分）：干净安装/启动/版本](windows-cli.md)
  - 证据：`test/aliasmgr-win-last-result/verification-summary.json`

- [x] W-002：添加原生别名
  - 类型：脚本验证
  - 当前结果：PASS
  - 详细说明：[windows-cli.md — 完整 CLI 生命周期](windows-cli.md)
  - 证据：`test/aliasmgr-win-last-result/verification-summary.json`

- [x] W-002b：列表包含已添加的条目
  - 类型：脚本验证
  - 当前结果：PASS
  - 详细说明：[windows-cli.md — 完整 CLI 生命周期](windows-cli.md)
  - 证据：`test/aliasmgr-win-last-result/verification-summary.json`

- [x] W-003：列表/搜索/limit
  - 类型：脚本验证
  - 当前结果：PASS
  - 详细说明：[windows-cli.md — 搜索和标签分面](windows-cli.md)
  - 证据：`test/aliasmgr-win-last-result/verification-summary.json`

- [x] W-004：argv 空格/引号/CJK/反斜杠/通配符
  - 类型：脚本验证
  - 当前结果：PASS
  - 详细说明：[windows-cli.md — argv 边界测试](windows-cli.md)
  - 证据：`test/aliasmgr-win-last-result/verification-summary.json`

- [x] W-005：`{{args}}` 尾部/中间/非法/重复
  - 类型：脚本验证
  - 当前结果：PASS
  - 详细说明：[windows-cli.md — argv 边界测试](windows-cli.md)
  - 证据：`test/aliasmgr-win-last-result/verification-summary.json`

- [ ] W-006：标签分面单选/多选/清除
  - 类型：脚本验证
  - 当前结果：EXPECTED-LIMITATION（`--oneline` 参数不可用：`error: unexpected argument '--oneline' found`）
  - 详细说明：[windows-cli.md — 搜索和标签分面](windows-cli.md)
  - 证据：`test/aliasmgr-win-last-result/verification-summary.json`

- [x] W-007：JSON/TOML 导入
  - 类型：脚本验证
  - 当前结果：PASS
  - 详细说明：[windows-cli.md](windows-cli.md)
  - 证据：`test/aliasmgr-win-last-result/verification-summary.json`

- [ ] W-008：retain/purge 卸载
  - 类型：脚本验证
  - 当前结果：EXPECTED-LIMITATION（`--dry-run` 参数不可用：`error: unexpected argument '--dry-run' found`）
  - 详细说明：[windows-cli.md](windows-cli.md)
  - 证据：`test/aliasmgr-win-last-result/verification-summary.json`

- [x] W-009：删除别名（CRUD 完成）
  - 类型：脚本验证
  - 当前结果：PASS
  - 详细说明：[windows-cli.md — 完整 CLI 生命周期](windows-cli.md)
  - 证据：`test/aliasmgr-win-last-result/verification-summary.json`

- [x] W-011：ACL/目标保护——非法名称被拒绝
  - 类型：脚本验证
  - 当前结果：PASS
  - 详细说明：[windows-cli.md](windows-cli.md)
  - 证据：`test/aliasmgr-win-last-result/verification-summary.json`

---

## Windows CLI 人工验证

以下条目来自 `verify-cli-windows.ps1` 中标记为手动或需要真实 Windows 环境的案例。不得用 CI 或脚本结果代替。

- [ ] W-MANUAL-001：Profile/OneDrive 路径解析
  - 类型：人工验证
  - 当前结果：EXPECTED-LIMITATION（`MANUAL-ONLY: OneDrive Profile path requires live Windows environment check`）
  - 详细说明：[windows-cli.md](windows-cli.md)
  - 证据：执行后填写 `docs/task21-2/test-matrix.md`

- [ ] W-MANUAL-002：BOM/CRLF 编码保留
  - 类型：人工验证
  - 当前结果：EXPECTED-LIMITATION（`MANUAL-ONLY: BOM/CRLF encoding verification requires a generated PowerShell loader file on a real artifact`）
  - 详细说明：[windows-cli.md](windows-cli.md)
  - 证据：执行后填写 `docs/task21-2/test-matrix.md`

---

## CLI 人工验证

以下条目涵盖脚本无法覆盖的手动验证场景：真实干净机器生命周期、Python/JAR/切换目录目标类型、升级/回滚，以及 `test-matrix.md` 中未被上述脚本覆盖的矩阵案例。

- [ ] L-003（人工）：Python/JAR/切换目录目标类型
  - 类型：人工验证
  - 当前结果：待执行
  - 详细说明：[linux-cli.md — L-003：Python/JAR/切换目录目标类型](linux-cli.md)
  - 证据：执行后填写 `docs/task21-2/test-matrix.md`

- [ ] L-018（人工）：retain/purge 卸载（完整手动步骤）
  - 类型：人工验证
  - 当前结果：待执行
  - 详细说明：[linux-cli.md — L-018/L-019：卸载、升级和回滚](linux-cli.md)
  - 证据：执行后填写 `docs/task21-2/test-matrix.md`

- [ ] L-019（人工）：升级和回滚（完整手动步骤）
  - 类型：人工验证
  - 当前结果：待执行
  - 详细说明：[linux-cli.md — L-018/L-019：卸载、升级和回滚](linux-cli.md)
  - 证据：执行后填写 `docs/task21-2/test-matrix.md`

---

## 提交结果

完成上述所有步骤后，请将结果填写至：

- 测试矩阵：[test-matrix.md](../test-matrix.md)
- 反馈模板（FAIL/BLOCKED/EXPECTED-LIMITATION）：[feedback-template.md](../feedback-template.md)

提交前确认：

- 脚本摘要路径（`result/verification-summary.json`）已脱敏，无用户名、完整路径、密码或 Token。
- 日志路径（`result/verification.log`）已脱敏。
- argv 摘要路径（`result/argv-summary.json`，如适用）已脱敏。
- 截图路径（如适用）已脱敏。
