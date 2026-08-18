# Windows 安装包验收手册

涵盖案例：PS51-010、PKG-001（Windows 部分）、PKG-002（Windows 部分）、PKG-003（Windows 部分）、PKG-004（Windows 部分）

**只有在真实 Windows 安装包 artifact（.msi 或 .exe）存在时执行。**

如果没有安装包 artifact，在矩阵中标记 `NOT-APPLICABLE` 并写明"Windows MSI/EXE artifact 在此 prerelease 阶段未提供"。

---

## PS51-010：MSI/安装包 hook（PS51-010 专属说明）

本手册涵盖测试矩阵中的 PS51-010 案例。PS51-010 验证 Windows 安装器/卸载器在 retain 和 purge 边界下的行为，与 PowerShell 5.1 Shell 集成验证（[../02-shell/windows-powershell51.md](../02-shell/windows-powershell51.md) 中的 PS51-009）独立执行。执行本手册中的 PKG-001 至 PKG-004 即覆盖 PS51-010 所需验证范围；请在矩阵 PS51-010 行填写对应结果。

---

## 准备工作

1. 确认有临时 Windows 账户（专用测试用户），不要在主账户执行安装测试。
2. 打开 PowerShell（任意版本），校验安装包 artifact SHA256：

   ```powershell
   Get-FileHash .\aliasmgr-setup.msi -Algorithm SHA256 | Select-Object Hash
   ```

   与 `SHA256SUMS` 文件中的对应值比对。如不匹配，停止测试并标记 `FAIL`。

3. 保存无关 Profile/配置的前后快照：

   ```powershell
   $beforeSnapshot = "$env:TEMP\before-install-snapshot.txt"
   Get-ChildItem "$env:APPDATA\aliasmgr" -ErrorAction SilentlyContinue | Out-File $beforeSnapshot
   Get-ChildItem "$env:LOCALAPPDATA\aliasmgr" -ErrorAction SilentlyContinue | Add-Content $beforeSnapshot
   ```

---

## PKG-001：干净安装包安装

### 步骤

1. 以普通用户身份安装（不要以管理员身份，除非安装程序要求提权）：

   ```
   [双击 aliasmgr-setup.msi 或 aliasmgr-setup.exe]
   ```

2. 安装向导流程（使用默认选项，不自定义路径）：
   - 点击 **Next** → 接受许可协议 → 点击 **Next** → 保持默认安装路径 → 点击 **Install**。
   - 如果 UAC 弹出，记录是否需要管理员权限（标记 `EXPECTED-LIMITATION` 如有）。

3. 安装完成后，启动应用并检查版本：

   ```powershell
   aliasmgr --version
   # 或通过开始菜单快捷方式启动
   ```

4. 检查无关 Profile/配置未被修改：

   ```powershell
   $afterSnapshot = "$env:TEMP\after-install-snapshot.txt"
   Get-ChildItem "$env:APPDATA\aliasmgr" -ErrorAction SilentlyContinue | Out-File $afterSnapshot
   # 比较 before 和 after snapshot（只比较无关配置目录）
   ```

### 预期

- 安装不修改无关配置；应用启动并报告正确版本；配置根目录如文档所述。

---

## PKG-002：升级和回滚

### 步骤

1. 确认已安装旧版本（如果有旧候选 artifact，先安装旧版本）。

2. 创建临时别名（升级前）：

   ```powershell
   aliasmgr add --name pkg-upgrade-test --target "Write-Output pre-upgrade" --shell powershell
   ```

3. 安装当前候选版本（升级）：

   ```
   [双击新版 MSI/EXE，选择升级]
   ```

4. 升级后验证：

   ```powershell
   aliasmgr --version
   aliasmgr list  # 确认 pkg-upgrade-test 保留
   aliasmgr get pkg-upgrade-test  # 确认数据完整
   ```

5. 检查备份和 schema 兼容性：

   ```powershell
   Get-ChildItem "$env:APPDATA\aliasmgr" -Recurse | Where-Object { $_.Name -like "*backup*" }
   ```

6. 如果支持回滚，执行回滚并重复步骤 4 的检查。如果不支持，记录 `NOT-APPLICABLE`。

### 预期

- 升级保留配置和别名；回滚恢复文档化状态；目标文件保留。

---

## PKG-003：postrm/MSI 卸载 hook

### 步骤

1. 准备引用临时目标的别名：

   ```powershell
   "# pkg hook test target" | Set-Content "$env:TEMP\pkg-hook-target.ps1" -Encoding UTF8
   aliasmgr add --name pkg-hook-test --target "pwsh -NoProfile -File $env:TEMP\pkg-hook-target.ps1" --shell powershell
   ```

2. 通过安装程序卸载（控制面板 → 程序和功能 → 卸载）：

   - 如果卸载时有选项（retain/purge），选择 **Retain**。
   - 确认卸载过程是**非交互式**的（不等待用户额外输入）。

3. 卸载完成后验证：

   ```powershell
   # 目标文件保留
   Test-Path "$env:TEMP\pkg-hook-target.ps1"

   # 无关 Profile 行未被删除
   # 比较 before snapshot 和当前状态
   ```

### 预期

- hook 不在包管理器上下文中等待输入；不删除目标文件；不删除无关 Profile 行；必要时引导用户显式执行 Alias Manager 清理。

### 缺少 artifact 的处理

如果没有 MSI/EXE：

- 在矩阵 PKG-003 行填写：结果 `NOT-APPLICABLE`，说明"Windows MSI artifact 在此 prerelease 阶段未提供，无法测试 MSI hook 行为"。

---

## PKG-004：非托管内容

### 步骤

1. 在 Profile 和无关配置目录放置 sentinel 内容（安装前）：

   ```powershell
   "# sentinel line - should not be modified" | Add-Content "$env:TEMP\test-profile-sentinel.ps1"
   ```

2. 依次执行安装、升级（如适用）、卸载和 purge。

3. 每步操作后比较脱敏 hash：

   ```powershell
   Get-FileHash "$env:TEMP\test-profile-sentinel.ps1" -Algorithm SHA256 | Select-Object Hash
   ```

4. 记录 hash 是否在每步后保持不变。

### 预期

- 非托管内容在所有操作后保持不变；任何文档化规范化必须明确报告。

---

## 保存结果日志

```powershell
$logPath = "$env:TEMP\aliasmgr-pkg-result.log"
"Windows installer smoke log" | Set-Content $logPath -Encoding UTF8
"Timestamp: $(Get-Date -Format 'yyyy-MM-ddTHH:mm:ssZ' -AsUTC)" | Add-Content $logPath
"Installer SHA256: [从 SHA256SUMS 文件填入]" | Add-Content $logPath
"OS: $([System.Environment]::OSVersion.VersionString)" | Add-Content $logPath
```

---

## 清理

```powershell
Remove-Item "$env:TEMP\before-install-snapshot.txt" -Force -ErrorAction SilentlyContinue
Remove-Item "$env:TEMP\after-install-snapshot.txt" -Force -ErrorAction SilentlyContinue
Remove-Item "$env:TEMP\pkg-hook-target.ps1" -Force -ErrorAction SilentlyContinue
Remove-Item "$env:TEMP\test-profile-sentinel.ps1" -Force -ErrorAction SilentlyContinue
```
