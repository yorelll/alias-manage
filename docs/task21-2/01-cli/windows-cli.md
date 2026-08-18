# Windows CLI 冒烟测试手册

涵盖案例：PS51-001（CLI 部分）、PS7-001（CLI 部分）

在开始之前，请先完成 [artifact-smoke.md](artifact-smoke.md) 中的下载和校验步骤。

本手册使用 PowerShell（5.1 或 7 均可）执行 CLI artifact 冒烟测试。Shell 集成（Profile、loader、ExecutionPolicy 等）的测试在 [../02-shell/windows-powershell51.md](../02-shell/windows-powershell51.md) 和 [../02-shell/windows-powershell7.md](../02-shell/windows-powershell7.md) 中单独覆盖。

---

## 准备工作

### 打开终端

1. 点击开始菜单，搜索 **Windows PowerShell** 或 **PowerShell 7**，以普通用户（不要以管理员）身份运行。
2. 确认当前用户是专用测试用户，**不要使用主账户的生产配置**。

### 准备测试目录

```powershell
$SmokeRoot = "$env:TEMP\aliasmgr-w-smoke"
$SmokeBin  = "$SmokeRoot\bin"
$SmokeCfg  = "$SmokeRoot\config"
$SmokeResult = "$SmokeRoot\result"
$SmokeTarget = "$SmokeRoot\targets"

New-Item -ItemType Directory -Force -Path $SmokeBin, $SmokeCfg, $SmokeResult, $SmokeTarget | Out-Null
```

### 将 artifact 复制到测试目录

```powershell
Copy-Item "$env:TEMP\aliasmgr-smoke\aliasmgr.exe" "$SmokeBin\aliasmgr.exe"
```

---

## PS51-001 / PS7-001（CLI 部分）：干净安装/启动/版本

### 步骤

1. 运行版本检查：

   ```powershell
   & "$SmokeBin\aliasmgr.exe" --version
   ```

2. 记录版本字符串（格式类似 `aliasmgr X.Y.Z`）。

3. 确认临时配置目录为空：

   ```powershell
   Get-ChildItem $SmokeCfg
   ```

4. 确认真实配置目录（`$env:APPDATA\aliasmgr` 或 `$env:LOCALAPPDATA\aliasmgr`）未被修改：

   ```powershell
   Test-Path "$env:APPDATA\aliasmgr"
   ```

### 预期

- 命令输出版本号。
- 无关的真实配置目录未被创建或修改。

---

## 完整 CLI 生命周期

### 步骤

1. 添加原生别名：

   ```powershell
   & "$SmokeBin\aliasmgr.exe" add `
     --config-dir $SmokeCfg `
     --name smoke-test-native `
     --target "Write-Output hello-world" `
     --shell powershell `
     --description "冒烟测试：原生别名"
   ```

2. 列出全部别名：

   ```powershell
   & "$SmokeBin\aliasmgr.exe" list --config-dir $SmokeCfg
   ```

3. 精确获取：

   ```powershell
   & "$SmokeBin\aliasmgr.exe" get --config-dir $SmokeCfg smoke-test-native
   ```

4. 修改描述：

   ```powershell
   & "$SmokeBin\aliasmgr.exe" edit `
     --config-dir $SmokeCfg `
     --name smoke-test-native `
     --description "冒烟测试：已修改"
   ```

5. 删除别名：

   ```powershell
   & "$SmokeBin\aliasmgr.exe" delete --config-dir $SmokeCfg smoke-test-native
   ```

6. 验证已删除：

   ```powershell
   & "$SmokeBin\aliasmgr.exe" list --config-dir $SmokeCfg
   ```

### 预期

- 每个操作返回正确状态；删除后列表为空或只剩其他别名。

---

## argv 边界测试

### 步骤

1. 创建 argument dumper 脚本：

   ```powershell
   @'
   param([string[]]$Args_)
   foreach ($a in $Args_) { Write-Output $a }
   '@ | Set-Content -Path "$SmokeTarget\argv-dump.ps1" -Encoding UTF8
   ```

2. 添加测试别名：

   ```powershell
   & "$SmokeBin\aliasmgr.exe" add `
     --config-dir $SmokeCfg `
     --name smoke-argv `
     --target "pwsh -NoProfile -File $SmokeTarget\argv-dump.ps1" `
     --shell powershell
   ```

3. 在预览模式测试特殊参数：

   ```powershell
   & "$SmokeBin\aliasmgr.exe" preview `
     --config-dir $SmokeCfg `
     smoke-argv -- "" "hello world" "你好" 'has"quote'
   ```

4. 记录每个参数的预览输出到 `$SmokeResult\argv-summary.json`。

### 预期

- 空字符串、空格、引号、CJK 字符保留；已知 PowerShell 宿主重构限制标记 `EXPECTED-LIMITATION`。

---

## 搜索和标签分面

### 步骤

1. 添加若干带不同标签的别名（可复用上面步骤中创建的别名）。

2. 文本搜索：

   ```powershell
   & "$SmokeBin\aliasmgr.exe" list --config-dir $SmokeCfg --filter "smoke"
   ```

3. 标签单选：

   ```powershell
   & "$SmokeBin\aliasmgr.exe" list --config-dir $SmokeCfg --tag "smoke"
   ```

4. 清除筛选，列出全部：

   ```powershell
   & "$SmokeBin\aliasmgr.exe" list --config-dir $SmokeCfg
   ```

### 预期

- 多标签为 AND；清除筛选恢复全部。

---

## 保存结果日志

```powershell
$logPath = "$SmokeResult\verification.log"
"Windows CLI smoke log" | Set-Content $logPath -Encoding UTF8
"Timestamp: $(Get-Date -Format 'yyyy-MM-ddTHH:mm:ssZ' -AsUTC)" | Add-Content $logPath
"PowerShell version: $($PSVersionTable.PSVersion)" | Add-Content $logPath
"Artifact SHA256: [从 SHA256SUMS 文件填入]" | Add-Content $logPath
"OS: $([System.Environment]::OSVersion.VersionString)" | Add-Content $logPath
```

将 `$SmokeResult\` 目录中的文件作为证据附件提交，提交前确认无用户名、完整路径、密码或 Token。

---

## 清理

```powershell
Remove-Item -Recurse -Force $SmokeRoot
```
