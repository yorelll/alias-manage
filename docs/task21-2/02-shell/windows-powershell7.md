# Windows PowerShell 7 Shell 集成验证手册

涵盖案例：PS7-002 至 PS7-009

**必须在 `pwsh`（PowerShell 7）下独立执行，不能复用 PowerShell 5.1 结果。**

在开始之前，请先完成 [../01-cli/windows-cli.md](../01-cli/windows-cli.md) 中的 CLI 冒烟测试。

---

## 打开正确的终端

1. 点击开始菜单，搜索 **PowerShell 7** 或 **pwsh**（不是 Windows PowerShell 5.1）。
2. 以**普通用户**身份运行。
3. 确认 Shell 版本：

   ```powershell
   $PSVersionTable.PSVersion
   ```

   主版本号必须为 `7` 或更高。如果显示 `5`，停止操作——你打开的是错误的 Shell。

4. 记录完整版本字符串（包括次版本和修订号）。

---

## 准备临时目录

```powershell
$PS7Smoke    = "$env:TEMP\aliasmgr-ps7-smoke"
$PS7Cfg      = "$PS7Smoke\config"
$PS7Profile  = "$PS7Smoke\profile.ps1"
$PS7Result   = "$PS7Smoke\result"
$PS7Gen      = "$PS7Smoke\generated"
$PS7Target   = "$PS7Smoke\targets"

New-Item -ItemType Directory -Force -Path $PS7Cfg, $PS7Result, $PS7Gen, $PS7Target | Out-Null
New-Item -ItemType File -Force -Path $PS7Profile | Out-Null
```

**重要：** 始终使用 `$PS7Profile`（临时 Profile），绝不修改真实的 `$PROFILE`。

---

## 记录 ExecutionPolicy 和语言模式（PS7-006 前置）

```powershell
Get-ExecutionPolicy -List | ForEach-Object {
  "[{0}] {1}" -f $_.Scope, $_.ExecutionPolicy
}
$ExecutionContext.SessionState.LanguageMode
```

---

## PS7-001/PS7-002：干净启动和完整生命周期

### 步骤

1. 确认 CLI 在 PS 7 下正常启动（使用临时配置目录）：

   ```powershell
   & aliasmgr --version
   ```

2. 独立完成增删改查：

   ```powershell
   # 创建
   & aliasmgr add `
     --config-dir $PS7Cfg `
     --name ps7-native `
     --target "Write-Output hello-ps7" `
     --shell powershell `
     --description "PS7 生命周期测试"

   # 列出
   & aliasmgr list --config-dir $PS7Cfg

   # 修改
   & aliasmgr edit --config-dir $PS7Cfg --name ps7-native --description "已修改"

   # 搜索
   & aliasmgr list --config-dir $PS7Cfg --filter "ps7"

   # 标签
   & aliasmgr edit --config-dir $PS7Cfg --name ps7-native --tag "ps7" --tag "smoke"
   & aliasmgr list --config-dir $PS7Cfg --tag "ps7"

   # 同步
   & aliasmgr sync --config-dir $PS7Cfg --generated-dir $PS7Gen

   # 禁用
   & aliasmgr disable --config-dir $PS7Cfg ps7-native

   # 删除
   & aliasmgr delete --config-dir $PS7Cfg ps7-native
   ```

3. 新建 PS 7 会话（关闭并重新打开 pwsh），重复持久化检查：

   ```powershell
   & aliasmgr list --config-dir $PS7Cfg
   ```

### 预期

- PS 7 状态独立持久化；与 PS 5.1 数据完全隔离（使用不同临时目录）。

---

## PS7-003：Profile 与 OneDrive

### 步骤

1. 记录 PS 7 Profile 路径（脱敏）：

   ```powershell
   $PROFILE.CurrentUserAllHosts -replace $env:USERNAME, "<USER>"
   ```

   PS 7 Profile 路径与 PS 5.1 不同，通常包含 `PowerShell` 而非 `WindowsPowerShell`。

2. 测试 PS 7 临时 Profile 覆盖：

   ```powershell
   & aliasmgr loader install `
     --config-dir $PS7Cfg `
     --profile-file $PS7Profile `
     --generated-dir $PS7Gen
   ```

3. 如果 Documents 被 OneDrive 重定向，记录实际解析路径（脱敏），重复步骤 2。

4. 确认真实 PS 7 Profile 未被修改：

   ```powershell
   Test-Path $PROFILE.CurrentUserAllHosts
   ```

### 预期

- PS 7 与 PS 5.1 路径独立；覆盖路径优先；真实无关 Profile 不被修改。

---

## PS7-004：编码和行尾

### 步骤

1. 创建带 BOM/no-BOM 变体：

   ```powershell
   # 无 BOM（PS 7 默认）
   [System.IO.File]::WriteAllText(
     "$PS7Smoke\profile-nobom.ps1",
     "# test profile no BOM`n",
     [System.Text.UTF8Encoding]::new($false)  # $false = 无 BOM
   )

   # 带 BOM
   [System.IO.File]::WriteAllText(
     "$PS7Smoke\profile-bom.ps1",
     "# test profile BOM`r`n",
     [System.Text.UTF8Encoding]::new($true)
   )
   ```

2. 对每个变体安装 loader，只记录字节摘要：

   ```powershell
   foreach ($f in @("profile-nobom.ps1", "profile-bom.ps1")) {
     $bytes = [System.IO.File]::ReadAllBytes("$PS7Smoke\$f")
     "File: $f | BOM: $($bytes[0] -eq 0xEF) | Size: $($bytes.Length)"
   }
   ```

3. 检查生成文件编码（PS 7 预期为无 BOM UTF-8）：

   ```powershell
   $genBytes = [System.IO.File]::ReadAllBytes("$PS7Gen\aliases.ps1")
   "Generated BOM: $($genBytes[0] -eq 0xEF)"
   ```

### 预期

- Profile 字节保持；PS 7 生成文件遵循文档化无 BOM 策略（与 PS 5.1 行为可能不同）。

---

## PS7-005：抢占和保留名

与 PS51-005 完全独立执行。

### 步骤

1. 测试 `ls` 抢占（使用 PS7 临时配置）：

   ```powershell
   & aliasmgr add `
     --config-dir $PS7Cfg `
     --name ls `
     --target "Write-Output custom-ls-ps7" `
     --shell powershell

   & aliasmgr sync --config-dir $PS7Cfg --generated-dir $PS7Gen
   ```

2. 测试 `cp` 保留名（期望被拒绝或报告 `NameReserved`）：

   ```powershell
   & aliasmgr add `
     --config-dir $PS7Cfg `
     --name Copy-Item `
     --target "Write-Output test" `
     --shell powershell
   ```

3. 记录 Get-Command 摘要（只记录类型/来源）：

   ```powershell
   Get-Command ls | Select-Object CommandType, Name, Source
   ```

### 预期

- 可移除 alias/function 被清理；保留名被拒绝；用户定义不被静默删除。

---

## PS7-006：策略和语言模式

与 PS51-006 完全独立执行。

### 步骤

1. 报告已记录的策略（使用准备步骤中的输出）。

2. 运行 Doctor，检查策略指引：

   ```powershell
   & aliasmgr doctor --config-dir $PS7Cfg
   ```

3. 不使用 `-ExecutionPolicy Bypass`，不修改 Group Policy。

4. 如果存在 ConstrainedLanguage，记录状态：

   ```powershell
   $ExecutionContext.SessionState.LanguageMode
   ```

### 预期

- Doctor 指引准确；限制或阻塞状态明确记录；`EXPECTED-LIMITATION` 明确标注。

---

## PS7-007：原生 argv

与 PS51-007 完全独立执行。

### 步骤

1. 创建 argument dumper 脚本（PS7 版本）：

   ```powershell
   @'
   foreach ($a in $args) { Write-Output $a }
   '@ | Set-Content "$PS7Target\argv-dump.ps1" -Encoding UTF8
   ```

2. 添加 argv 测试别名：

   ```powershell
   & aliasmgr add `
     --config-dir $PS7Cfg `
     --name ps7-argv `
     --target "pwsh -NoProfile -File $PS7Target\argv-dump.ps1" `
     --shell powershell
   ```

3. 预览模式测试边界参数：

   ```powershell
   & aliasmgr preview --config-dir $PS7Cfg ps7-argv -- "" "hello world" "你好" "trailing\"
   ```

4. 如果系统有多个 PS 7 版本（如 7.2 和 7.4），分别记录结果。

5. 将逐元素比较记录到 `$PS7Result\argv-summary.json`。

---

## PS7-008：ACL 和目标保护

### 步骤

1. 创建安全临时目标：

   ```powershell
   "# ps7 safe target" | Set-Content "$PS7Target\safe-ps7.ps1" -Encoding UTF8
   ```

2. 添加引用该目标的别名并检查安全状态：

   ```powershell
   & aliasmgr add `
     --config-dir $PS7Cfg `
     --name ps7-safe `
     --target "pwsh -NoProfile -File $PS7Target\safe-ps7.ps1" `
     --shell powershell
   & aliasmgr get --config-dir $PS7Cfg ps7-safe
   ```

3. 执行 purge 卸载后确认目标字节未变：

   ```powershell
   $before = (Get-FileHash "$PS7Target\safe-ps7.ps1" -Algorithm SHA256).Hash
   & aliasmgr uninstall --config-dir $PS7Cfg --mode purge
   $after = (Get-FileHash "$PS7Target\safe-ps7.ps1" -Algorithm SHA256).Hash
   "Hashes match: $($before -eq $after)"
   ```

---

## PS7-009：loader/卸载

### 步骤

1. 连续安装 loader 两次（测试幂等性）：

   ```powershell
   & aliasmgr loader install --config-dir $PS7Cfg --profile-file $PS7Profile --generated-dir $PS7Gen
   & aliasmgr loader install --config-dir $PS7Cfg --profile-file $PS7Profile --generated-dir $PS7Gen
   ```

2. 确认 marker 只有一个：

   ```powershell
   (Select-String -Path $PS7Profile -Pattern "aliasmgr").Count
   ```

3. 分别测试 retain 和 purge 卸载，验证无关内容保留：

   ```powershell
   & aliasmgr uninstall --config-dir $PS7Cfg --mode retain
   Get-ChildItem $PS7Target
   ```

### 预期

- Marker 修改幂等；不删除非托管内容；retain 保留配置数据。

---

## 保存结果日志

```powershell
$logPath = "$PS7Result\verification.log"
"Windows PS 7 Shell integration log" | Set-Content $logPath -Encoding UTF8
"Timestamp: $(Get-Date -Format 'yyyy-MM-ddTHH:mm:ssZ' -AsUTC)" | Add-Content $logPath
"PSVersion: $($PSVersionTable.PSVersion)" | Add-Content $logPath
"OS: $([System.Environment]::OSVersion.VersionString)" | Add-Content $logPath
"Artifact SHA256: [从 SHA256SUMS 文件填入]" | Add-Content $logPath
```

---

## 清理

```powershell
Remove-Item -Recurse -Force $PS7Smoke
```
