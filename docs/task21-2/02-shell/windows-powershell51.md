# Windows PowerShell 5.1 Shell 集成验证手册

涵盖案例：PS51-002 至 PS51-009

**必须在 Windows PowerShell 5.1 下独立执行，不能用 `pwsh` 代替。**

在开始之前，请先完成 [../01-cli/windows-cli.md](../01-cli/windows-cli.md) 中的 CLI 冒烟测试。

---

## 打开正确的终端

1. 点击开始菜单，搜索 **Windows PowerShell**（不是 PowerShell 7，不是 pwsh）。
2. 以**普通用户**身份运行（不要以管理员身份运行，除非测试 ACL 案例需要）。
3. 确认 Shell 版本：

   ```powershell
   $PSVersionTable.PSVersion
   ```

   主版本号必须为 `5`。如果显示 `7` 或更高，停止操作——你打开的是错误的 Shell。

4. 记录完整版本字符串。

---

## 准备临时目录

```powershell
$PS51Smoke    = "$env:TEMP\aliasmgr-ps51-smoke"
$PS51Cfg      = "$PS51Smoke\config"
$PS51Profile  = "$PS51Smoke\profile.ps1"
$PS51Result   = "$PS51Smoke\result"
$PS51Gen      = "$PS51Smoke\generated"
$PS51Target   = "$PS51Smoke\targets"

New-Item -ItemType Directory -Force -Path $PS51Cfg, $PS51Result, $PS51Gen, $PS51Target | Out-Null
New-Item -ItemType File -Force -Path $PS51Profile | Out-Null
```

**重要：** 整个测试过程中，只修改 `$PS51Profile`（临时 Profile 文件），绝不修改真实的 `$PROFILE`。

---

## 记录 ExecutionPolicy（PS51-006 前置）

测试开始前，记录当前策略（仅摘要，不暴露私有内容）：

```powershell
Get-ExecutionPolicy -List | ForEach-Object {
  "[{0}] {1}" -f $_.Scope, $_.ExecutionPolicy
}
$ExecutionContext.SessionState.LanguageMode
```

保存输出，用于 PS51-006 和 PS51-007 的记录。

---

## PS51-002：完整生命周期

### 步骤

1. 创建原生别名（使用临时配置目录）：

   ```powershell
   & aliasmgr add `
     --config-dir $PS51Cfg `
     --name ps51-native `
     --target "Write-Output hello-ps51" `
     --shell powershell `
     --description "PS5.1 生命周期测试"
   ```

2. 添加 PowerShell 脚本目标：

   ```powershell
   @'
   param([string[]]$Args_)
   Write-Output "PS5.1 script args: $Args_"
   '@ | Set-Content "$PS51Target\test-script.ps1" -Encoding UTF8

   & aliasmgr add `
     --config-dir $PS51Cfg `
     --name ps51-script `
     --target "powershell -NoProfile -File $PS51Target\test-script.ps1" `
     --shell powershell
   ```

3. 完整生命周期操作：

   ```powershell
   # 列出
   & aliasmgr list --config-dir $PS51Cfg

   # 搜索
   & aliasmgr list --config-dir $PS51Cfg --filter "ps51"

   # 标签操作
   & aliasmgr edit --config-dir $PS51Cfg --name ps51-native --tag "ps51" --tag "smoke"
   & aliasmgr list --config-dir $PS51Cfg --tag "ps51"

   # 同步和 reload
   & aliasmgr sync --config-dir $PS51Cfg --generated-dir $PS51Gen

   # 禁用
   & aliasmgr disable --config-dir $PS51Cfg ps51-native

   # 删除
   & aliasmgr delete --config-dir $PS51Cfg ps51-script
   ```

4. 新建 PS 5.1 会话（关闭并重新打开 Windows PowerShell），重复关键检查：

   ```powershell
   & aliasmgr list --config-dir $PS51Cfg
   ```

### 预期

- 每步操作成功；状态持久化；reload 只移除托管残留。

---

## PS51-003：Profile 与 OneDrive

### 步骤

1. 记录真实 Profile 路径摘要（脱敏，不暴露用户名）：

   ```powershell
   # 只记录路径结构，不暴露用户名
   $PROFILE.CurrentUserAllHosts -replace $env:USERNAME, "<USER>"
   ```

2. 测试临时 Profile 覆盖（始终指向 `$PS51Profile`）：

   ```powershell
   & aliasmgr loader install `
     --config-dir $PS51Cfg `
     --profile-file $PS51Profile `
     --generated-dir $PS51Gen
   ```

3. 如果 Documents 目录被 OneDrive 重定向，记录实际解析路径（脱敏），并用该路径重复步骤 2。

4. 确认真实无关 Profile 未被修改：

   ```powershell
   Test-Path $PROFILE.CurrentUserAllHosts
   # 如果存在，只检查 hash 是否改变，不要输出文件内容
   ```

### 预期

- 使用解析后的 Profile；覆盖路径优先；真实无关 Profile 不被修改。

---

## PS51-004：BOM/CRLF

### 步骤

1. 创建带 BOM + CRLF 的临时 Profile：

   ```powershell
   # 创建带 UTF-8 BOM 的 Profile
   [System.IO.File]::WriteAllText(
     "$PS51Smoke\profile-bom.ps1",
     "# test profile with BOM`r`n",
     [System.Text.UTF8Encoding]::new($true)  # $true = 带 BOM
   )
   ```

2. 安装 loader 到该 Profile：

   ```powershell
   & aliasmgr loader install `
     --config-dir $PS51Cfg `
     --profile-file "$PS51Smoke\profile-bom.ps1" `
     --generated-dir $PS51Gen
   ```

3. 只记录编码和行尾摘要（不上传文件内容）：

   ```powershell
   $bytes = [System.IO.File]::ReadAllBytes("$PS51Smoke\profile-bom.ps1")
   "BOM present: $($bytes[0] -eq 0xEF -and $bytes[1] -eq 0xBB -and $bytes[2] -eq 0xBF)"
   "File size: $($bytes.Length) bytes"
   ```

4. 检查生成文件的编码：

   ```powershell
   $genBytes = [System.IO.File]::ReadAllBytes("$PS51Gen\aliases.ps1")
   "Generated BOM: $($genBytes[0] -eq 0xEF)"
   ```

### 预期

- 原 Profile 编码和行尾保留；PS 5.1 生成文件遵守文档化 BOM 策略。

---

## PS51-005：内置别名与保留名

### 步骤

1. 测试 `ls` 抢占（临时定义）：

   ```powershell
   & aliasmgr add `
     --config-dir $PS51Cfg `
     --name ls `
     --target "Write-Output custom-ls" `
     --shell powershell

   # 同步后查看 Get-Command 摘要
   & aliasmgr sync --config-dir $PS51Cfg --generated-dir $PS51Gen
   ```

2. 测试保留名（期望被拒绝）：

   ```powershell
   & aliasmgr add `
     --config-dir $PS51Cfg `
     --name Get-Item `
     --target "Write-Output test" `
     --shell powershell
   ```

3. 只记录类型/来源摘要（不上传完整 Get-Command 输出）：

   ```powershell
   Get-Command ls | Select-Object CommandType, Name, Source
   ```

### 预期

- 可移除冲突被抢占；保留定义返回 `NameReserved` 错误；不强制删除 Constant 定义。

---

## PS51-006：ExecutionPolicy/语言模式

### 步骤

1. 报告已记录的策略（使用准备步骤中的输出）。

2. 如果策略是 `Restricted` 或 `AllSigned`，运行 Doctor 检查预期错误：

   ```powershell
   & aliasmgr doctor --config-dir $PS51Cfg
   ```

3. 如果存在 Group Policy 或 ConstrainedLanguage，记录状态但不绕过策略：

   ```powershell
   $ExecutionContext.SessionState.LanguageMode
   ```

4. 不使用 `-ExecutionPolicy Bypass`，不修改 Group Policy。

### 预期

- Doctor 提供准确指引；不自动修改策略；`EXPECTED-LIMITATION` 或 `BLOCKED` 明确记录。

---

## PS51-007：原生 argv

### 步骤

1. 创建 argument dumper 脚本：

   ```powershell
   @'
   param()
   $Args | ForEach-Object { Write-Output $_ }
   '@ | Set-Content "$PS51Target\argv-dump.ps1" -Encoding UTF8
   ```

2. 添加 argv 测试别名：

   ```powershell
   & aliasmgr add `
     --config-dir $PS51Cfg `
     --name ps51-argv `
     --target "powershell -NoProfile -File $PS51Target\argv-dump.ps1" `
     --shell powershell
   ```

3. 在预览模式测试边界（不实际运行目标）：

   ```powershell
   & aliasmgr preview `
     --config-dir $PS51Cfg `
     ps51-argv -- "" "hello world" "你好" 'has"quote' "trailing\"
   ```

4. 逐元素记录预期和实际结果到 `$PS51Result\argv-summary.json`。

5. 已知 PS 5.1 宿主参数重构限制标记为 `EXPECTED-LIMITATION`。

---

## PS51-008：ACL/目标保护

### 步骤

1. 创建安全临时目标（用户 owned）：

   ```powershell
   "# safe target" | Set-Content "$PS51Target\safe-target.ps1" -Encoding UTF8
   ```

2. 添加引用安全目标的别名并检查警告：

   ```powershell
   & aliasmgr add `
     --config-dir $PS51Cfg `
     --name ps51-safe-target `
     --target "powershell -NoProfile -File $PS51Target\safe-target.ps1" `
     --shell powershell
   & aliasmgr get --config-dir $PS51Cfg ps51-safe-target
   ```

3. 测试不安全路径（若环境允许）：尝试添加引用其他用户可写路径的别名，记录警告或阻止行为。

4. 执行 purge 卸载，确认目标字节未改变：

   ```powershell
   $beforeHash = (Get-FileHash "$PS51Target\safe-target.ps1" -Algorithm SHA256).Hash
   & aliasmgr uninstall --config-dir $PS51Cfg --mode purge
   $afterHash = (Get-FileHash "$PS51Target\safe-target.ps1" -Algorithm SHA256).Hash
   "Hashes match: $($beforeHash -eq $afterHash)"
   ```

### 预期

- 未经验证的 ACL 不获得安全授权；目标字节不变。

---

## PS51-009：loader/卸载

### 步骤

1. 连续安装 loader 两次：

   ```powershell
   & aliasmgr loader install --config-dir $PS51Cfg --profile-file $PS51Profile --generated-dir $PS51Gen
   & aliasmgr loader install --config-dir $PS51Cfg --profile-file $PS51Profile --generated-dir $PS51Gen
   ```

2. 比较 Profile 中 marker 数量（应为 1）：

   ```powershell
   (Select-String -Path $PS51Profile -Pattern "aliasmgr").Count
   ```

3. 测试 retain 卸载：

   ```powershell
   & aliasmgr uninstall --config-dir $PS51Cfg --mode retain
   ```

4. 验证无关 Profile 内容和目标文件保留：

   ```powershell
   Get-ChildItem $PS51Target
   ```

### 预期

- Marker 操作幂等且安全；retain 保留配置数据；purge 清除配置但不删除目标。

---

## 保存结果日志

```powershell
$logPath = "$PS51Result\verification.log"
"Windows PS 5.1 Shell integration log" | Set-Content $logPath -Encoding UTF8
"Timestamp: $(Get-Date -Format 'yyyy-MM-ddTHH:mm:ssZ' -AsUTC)" | Add-Content $logPath
"PSVersion: $($PSVersionTable.PSVersion)" | Add-Content $logPath
"OS: $([System.Environment]::OSVersion.VersionString)" | Add-Content $logPath
"Artifact SHA256: [从 SHA256SUMS 文件填入]" | Add-Content $logPath
```

---

## 清理

```powershell
Remove-Item -Recurse -Force $PS51Smoke
```
