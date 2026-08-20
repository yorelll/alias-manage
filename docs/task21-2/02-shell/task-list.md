# 02-shell：Shell 验证任务清单

> 当前清单用于逐项跟踪。`[x]` 仅表示对应脚本案例结果为 `PASS`；人工项目、`EXPECTED-LIMITATION`、`NOT-APPLICABLE` 和探索性项目保持 `[ ]`，不得从 CI 推断人工 PASS。
>
> 结果枚举严格使用：`PASS | FAIL | BLOCKED | EXPECTED-LIMITATION | NOT-APPLICABLE`。

## 当前结果同步

当前结果来源：`test/aliasmgr-bash-last-result/`、`test/aliasmgr-zsh-last-result/`、`test/aliasmgr-ps51-last-result/`、`test/aliasmgr-ps7-last-result/`。

| Shell | 当前结果 | 结果元数据路径 |
|---|---|---|
| Linux Bash | 7 PASS + 4 EXPECTED-LIMITATION + 1 NOT-APPLICABLE | `/tmp/aliasmgr-bash-last-result/verification-summary.json` |
| Linux Zsh | 7 PASS + 5 EXPECTED-LIMITATION | `/tmp/aliasmgr-zsh-last-result/verification-summary.json` |
| Windows PowerShell 5.1 | 3 PASS + 7 EXPECTED-LIMITATION | `$env:TEMP\aliasmgr-ps51-last-result\verification-summary.json` |
| Windows PowerShell 7 | 4 PASS + 6 EXPECTED-LIMITATION | `$env:TEMP\aliasmgr-ps7-last-result\verification-summary.json` |

注：元数据路径表显示脚本运行时位置；每项证据使用仓库内相对路径 `test/`，方便审阅已保存结果。

---

## Linux Bash 脚本验证

脚本：[verify-bash-linux.sh](../../../scripts/verify-bash-linux.sh) · 手册：[linux-bash.md](linux-bash.md)

- [x] **B-ENV-001**：bash binary available — 类型：脚本验证；当前结果：`PASS`；详细说明：[linux-bash.md](linux-bash.md#l-009bash-loaderreload)；证据：`test/aliasmgr-bash-last-result/verification-summary.json`
- [x] **B-001**：bash loader install to isolated RC — 类型：脚本验证；当前结果：`PASS`；详细说明：[linux-bash.md](linux-bash.md#l-009bash-loaderreload)；证据：`test/aliasmgr-bash-last-result/verification-summary.json`
- [x] **B-002**：bash loader install idempotence — 类型：脚本验证；当前结果：`PASS`；详细说明：[linux-bash.md](linux-bash.md#l-009bash-loaderreload)；证据：`test/aliasmgr-bash-last-result/verification-summary.json`
- [x] **B-003**：bash reload guidance — 类型：脚本验证；当前结果：`PASS`；详细说明：[linux-bash.md](linux-bash.md#l-009bash-loaderreload)；证据：`test/aliasmgr-bash-last-result/verification-summary.json`
- [x] **B-004**：bash argv boundary — 类型：脚本验证；当前结果：`PASS`；详细说明：[linux-bash.md](linux-bash.md#l-011argv)；证据：`test/aliasmgr-bash-last-result/argv-summary.json`
- [x] **B-005**：bash tag/search — 类型：脚本验证；当前结果：`PASS`；详细说明：[linux-bash.md](linux-bash.md#l-012tagsearch)；证据：`test/aliasmgr-bash-last-result/verification-summary.json`
- [x] **B-006**：bash uninstall removes loader — 类型：脚本验证；当前结果：`PASS`；详细说明：[linux-bash.md](linux-bash.md#l-013uninstall)；证据：`test/aliasmgr-bash-last-result/verification-summary.json`

## Linux Bash 手动跟进

- [ ] **B-007**：generated bash syntax — 类型：人工/限制；当前结果：`EXPECTED-LIMITATION`；详细说明：[linux-bash.md](linux-bash.md)；证据：执行后填写 `../test-matrix.md`。
- [ ] **B-MANUAL-001**：真实 login chain — 类型：人工验证；当前结果：`EXPECTED-LIMITATION`；详细说明：[linux-bash.md](linux-bash.md)；证据：用户填写截图/日志。
- [ ] **B-MANUAL-002**：oh-my-zsh ordering — 类型：人工验证；当前结果：`NOT-APPLICABLE`；详细说明：[linux-bash.md](linux-bash.md)；证据：说明不适用于 Bash。
- [ ] **B-MANUAL-003**：trusted symlink RC — 类型：人工验证；当前结果：`EXPECTED-LIMITATION`；详细说明：[linux-bash.md](linux-bash.md)；证据：用户填写截图/日志。
- [ ] **B-MANUAL-004**：current session reload — 类型：人工验证；当前结果：`EXPECTED-LIMITATION`；详细说明：[linux-bash.md](linux-bash.md)；证据：用户填写截图/日志。

---

## Linux Zsh 脚本验证

脚本：[verify-zsh-linux.sh](../../../scripts/verify-zsh-linux.sh) · 手册：[linux-zsh.md](linux-zsh.md)

- [x] **Z-ENV-001**：zsh binary available — 类型：脚本验证；当前结果：`PASS`；详细说明：[linux-zsh.md](linux-zsh.md#l-010zsh-loaderreload)；证据：`test/aliasmgr-zsh-last-result/verification-summary.json`
- [x] **Z-001**：zsh loader install — 类型：脚本验证；当前结果：`PASS`；详细说明：[linux-zsh.md](linux-zsh.md#l-010zsh-loaderreload)；证据：`test/aliasmgr-zsh-last-result/verification-summary.json`
- [x] **Z-002**：zsh loader idempotence — 类型：脚本验证；当前结果：`PASS`；详细说明：[linux-zsh.md](linux-zsh.md#l-010zsh-loaderreload)；证据：`test/aliasmgr-zsh-last-result/verification-summary.json`
- [x] **Z-003**：zsh reload guidance — 类型：脚本验证；当前结果：`PASS`；详细说明：[linux-zsh.md](linux-zsh.md#l-010zsh-loaderreload)；证据：`test/aliasmgr-zsh-last-result/verification-summary.json`
- [x] **Z-005**：zsh argv boundary — 类型：脚本验证；当前结果：`PASS`；详细说明：[linux-zsh.md](linux-zsh.md#l-011argv)；证据：`test/aliasmgr-zsh-last-result/argv-summary.json`
- [x] **Z-006**：zsh tag filter — 类型：脚本验证；当前结果：`PASS`；详细说明：[linux-zsh.md](linux-zsh.md#l-012tag)；证据：`test/aliasmgr-zsh-last-result/verification-summary.json`
- [x] **Z-007**：zsh uninstall removes loader — 类型：脚本验证；当前结果：`PASS`；详细说明：[linux-zsh.md](linux-zsh.md#l-013uninstall)；证据：`test/aliasmgr-zsh-last-result/verification-summary.json`

## Linux Zsh 手动跟进

- [ ] **Z-004**：generated zsh syntax — 类型：人工/限制；当前结果：`EXPECTED-LIMITATION`；详细说明：[linux-zsh.md](linux-zsh.md)；证据：执行后填写 `../test-matrix.md`。
- [ ] **Z-MANUAL-001**：oh-my-zsh plugin ordering — 类型：人工验证；当前结果：`EXPECTED-LIMITATION`；详细说明：[linux-zsh.md](linux-zsh.md)；证据：用户填写截图/日志。
- [ ] **Z-MANUAL-002**：real login chain — 类型：人工验证；当前结果：`EXPECTED-LIMITATION`；详细说明：[linux-zsh.md](linux-zsh.md)；证据：用户填写截图/日志。
- [ ] **Z-MANUAL-003**：ZDOTDIR symlink RC — 类型：人工验证；当前结果：`EXPECTED-LIMITATION`；详细说明：[linux-zsh.md](linux-zsh.md)；证据：用户填写截图/日志。
- [ ] **Z-MANUAL-004**：current session reload — 类型：人工验证；当前结果：`EXPECTED-LIMITATION`；详细说明：[linux-zsh.md](linux-zsh.md)；证据：用户填写截图/日志。

---

## Windows PowerShell 5.1 脚本验证

脚本：[verify-powershell51.ps1](../../../scripts/verify-powershell51.ps1) · 手册：[windows-powershell51.md](windows-powershell51.md)

- [x] **PS51-001**：version and startup — 类型：脚本验证；当前结果：`PASS`；详细说明：[windows-powershell51.md](windows-powershell51.md#ps51-001)；证据：`test/aliasmgr-ps51-last-result/verification-summary.json`
- [x] **PS51-002**：full lifecycle — 类型：脚本验证；当前结果：`PASS`；详细说明：[windows-powershell51.md](windows-powershell51.md#ps51-002)；证据：`test/aliasmgr-ps51-last-result/verification-summary.json`
- [x] **PS51-006**：ExecutionPolicy report — 类型：脚本验证；当前结果：`PASS`；详细说明：[windows-powershell51.md](windows-powershell51.md#ps51-006)；证据：`test/aliasmgr-ps51-last-result/verification-summary.json`

## Windows PowerShell 5.1 手动/限制验证

- [ ] **PS51-003**：Profile/OneDrive — 类型：人工验证；当前结果：`EXPECTED-LIMITATION`；详细说明：[windows-powershell51.md](windows-powershell51.md#ps51-003)；证据：用户填写截图/日志。
- [ ] **PS51-004**：BOM/CRLF — 类型：人工/限制；当前结果：`EXPECTED-LIMITATION`；详细说明：[windows-powershell51.md](windows-powershell51.md#ps51-004)；证据：用户填写生成文件信息。
- [ ] **PS51-005**：内置别名抢占 — 类型：人工验证；当前结果：`EXPECTED-LIMITATION`；详细说明：[windows-powershell51.md](windows-powershell51.md#ps51-005)；证据：用户填写截图/日志。
- [ ] **PS51-007**：native argv — 类型：人工/限制；当前结果：`EXPECTED-LIMITATION`；详细说明：[windows-powershell51.md](windows-powershell51.md#ps51-007)；证据：用户填写 argv 观察结果。
- [ ] **PS51-008**：ACL/target protection — 类型：人工/限制；当前结果：`EXPECTED-LIMITATION`；详细说明：[windows-powershell51.md](windows-powershell51.md#ps51-008)；证据：用户填写日志。
- [ ] **PS51-009**：loader uninstall idempotence — 类型：人工/限制；当前结果：`EXPECTED-LIMITATION`；详细说明：[windows-powershell51.md](windows-powershell51.md#ps51-009)；证据：用户填写日志。
- [ ] **PS51-LANG-001**：ConstrainedLanguage status — 类型：人工/限制；当前结果：`EXPECTED-LIMITATION`；详细说明：[windows-powershell51.md](windows-powershell51.md)；证据：用户填写策略信息。

---

## Windows PowerShell 7 脚本验证

脚本：[verify-powershell7.ps1](../../../scripts/verify-powershell7.ps1) · 手册：[windows-powershell7.md](windows-powershell7.md)

- [x] **PS7-001**：version and startup — 类型：脚本验证；当前结果：`PASS`；详细说明：[windows-powershell7.md](windows-powershell7.md#ps7-001)；证据：`test/aliasmgr-ps7-last-result/verification-summary.json`
- [x] **PS7-002**：full lifecycle — 类型：脚本验证；当前结果：`PASS`；详细说明：[windows-powershell7.md](windows-powershell7.md#ps7-002)；证据：`test/aliasmgr-ps7-last-result/verification-summary.json`
- [x] **PS7-006**：ExecutionPolicy report — 类型：脚本验证；当前结果：`PASS`；详细说明：[windows-powershell7.md](windows-powershell7.md#ps7-006)；证据：`test/aliasmgr-ps7-last-result/verification-summary.json`
- [x] **PS7-008**：invalid name/target protection — 类型：脚本验证；当前结果：`PASS`；详细说明：[windows-powershell7.md](windows-powershell7.md#ps7-008)；证据：`test/aliasmgr-ps7-last-result/verification-summary.json`

## Windows PowerShell 7 手动/限制验证

- [ ] **PS7-003**：Profile/OneDrive — 类型：人工验证；当前结果：`EXPECTED-LIMITATION`；详细说明：[windows-powershell7.md](windows-powershell7.md#ps7-003)；证据：用户填写截图/日志。
- [ ] **PS7-004**：BOM/CRLF — 类型：人工/限制；当前结果：`EXPECTED-LIMITATION`；详细说明：[windows-powershell7.md](windows-powershell7.md#ps7-004)；证据：用户填写生成文件信息。
- [ ] **PS7-005**：内置别名抢占 — 类型：人工验证；当前结果：`EXPECTED-LIMITATION`；详细说明：[windows-powershell7.md](windows-powershell7.md#ps7-005)；证据：用户填写截图/日志。
- [ ] **PS7-007**：native argv — 类型：人工/限制；当前结果：`EXPECTED-LIMITATION`；详细说明：[windows-powershell7.md](windows-powershell7.md#ps7-007)；证据：用户填写 argv 观察结果。
- [ ] **PS7-009**：loader uninstall idempotence — 类型：人工/限制；当前结果：`EXPECTED-LIMITATION`；详细说明：[windows-powershell7.md](windows-powershell7.md#ps7-009)；证据：用户填写日志。
- [ ] **PS7-LANG-001**：ConstrainedLanguage status — 类型：人工/限制；当前结果：`EXPECTED-LIMITATION`；详细说明：[windows-powershell7.md](windows-powershell7.md)；证据：用户填写策略信息。

---

## Windows Git Bash 探索性验证

脚本：无正式自动化脚本 · 手册：[windows-git-bash.md](windows-git-bash.md)

- [ ] **GB-EXP-001**：MSYS2 path conversion — 类型：探索性；当前结果：`EXPECTED-LIMITATION`；详细说明：[windows-git-bash.md](windows-git-bash.md)；证据：不能替代 Linux Bash。
- [ ] **GB-EXP-002**：Windows native argv boundary — 类型：探索性；当前结果：`EXPECTED-LIMITATION`；详细说明：[windows-git-bash.md](windows-git-bash.md)；证据：另行记录观察结果。
- [ ] **GB-EXP-003**：loader/profile behavior — 类型：探索性；当前结果：`EXPECTED-LIMITATION`；详细说明：[windows-git-bash.md](windows-git-bash.md)；证据：另行记录观察结果。
- [ ] **GB-EXP-004**：classification — 类型：探索性；当前结果：`EXPECTED-LIMITATION`；详细说明：[windows-git-bash.md](windows-git-bash.md)；证据：必须标记为探索性。

---

## 提交结果

1. 将脚本生成的三个结果文件保留在结果目录，并确认没有用户名、完整路径、Token、密码或私有目标内容。
2. 将人工结果填写到 [`../test-matrix.md`](../test-matrix.md)，并对每个 `FAIL`、`BLOCKED`、`EXPECTED-LIMITATION` 项复制填写 [`../feedback-template.md`](../feedback-template.md)。
3. 只有用户完成人工步骤并填写 Actual、Result、Evidence 后，才可以勾选对应人工项目。
4. Windows Git Bash 不能替代 Linux Bash；PowerShell 5.1 和 7 结果不能互相替代。

## 安全提示

始终使用专用临时目录或专用测试用户，不要修改主账户真实 RC/Profile；不得提交完整配置文件或环境变量转储。
