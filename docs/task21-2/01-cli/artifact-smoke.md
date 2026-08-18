# artifact 下载、校验和清单说明

在开始任何测试之前，必须先下载候选 artifact 并校验其完整性。

---

## 第一步：找到候选 artifact

1. 打开浏览器，进入项目的 GitHub 仓库页面。
2. 点击顶部导航栏的 **Actions** 标签页。
3. 在左侧工作流列表中，找到最近一次成功的 CI 或 prerelease 工作流运行记录，点击进入。
4. 页面底部找到 **Artifacts** 区域，下载以下文件（根据平台选择）：
   - Linux x86_64：`aliasmgr-linux-x86_64.tar.gz` 或类似名称
   - Windows x86_64：`aliasmgr-windows-x86_64.zip` 或类似名称
5. 同时下载 `SHA256SUMS` 文件（与 artifact 在同一区域）。

---

## 第二步：校验 SHA256

### Linux / macOS 终端

打开终端，进入下载目录，运行：

```bash
sha256sum -c SHA256SUMS
```

预期输出：每个 artifact 文件后面显示 `OK`。

如果显示 `FAILED`，立即停止测试并标记 `FAIL`，不要使用该 artifact。

### Windows PowerShell

打开 PowerShell（任意版本），进入下载目录，运行：

```powershell
Get-FileHash .\aliasmgr-windows-x86_64.zip -Algorithm SHA256 | Select-Object Hash
```

将输出的哈希值（忽略大小写）与 `SHA256SUMS` 文件中对应行的哈希值手动比对。两者必须完全一致。

---

## 第三步：解压 artifact

### Linux

```bash
mkdir -p /tmp/aliasmgr-smoke
tar -xzf aliasmgr-linux-x86_64.tar.gz -C /tmp/aliasmgr-smoke
ls /tmp/aliasmgr-smoke
```

### Windows PowerShell

```powershell
$smokeDir = "$env:TEMP\aliasmgr-smoke"
New-Item -ItemType Directory -Force -Path $smokeDir | Out-Null
Expand-Archive -Path .\aliasmgr-windows-x86_64.zip -DestinationPath $smokeDir -Force
Get-ChildItem $smokeDir
```

---

## 第四步：记录版本和路径信息

执行以下命令，记录版本输出：

### Linux

```bash
/tmp/aliasmgr-smoke/aliasmgr --version
```

### Windows PowerShell

```powershell
& "$env:TEMP\aliasmgr-smoke\aliasmgr.exe" --version
```

将版本字符串和 artifact SHA256 填入测试矩阵的对应记录字段。

---

## 产出文件说明

冒烟脚本（在 CLI 手册中引用）会在 `result/` 子目录下生成以下文件：

| 文件 | 说明 |
|---|---|
| `result/verification-summary.json` | 结构化结果摘要，包含每个案例的 id / result / expected / actual / evidence 字段 |
| `result/verification.log` | 完整执行日志（已脱敏，不含完整路径或私有内容） |
| `result/argv-summary.json` | argv 边界测试的逐元素对比记录 |

这些文件是反馈提交时的证据附件。提交前请确认文件中无用户名、完整路径、密码、Token 或其他敏感内容。

---

## 清理

测试完成后，删除临时目录：

### Linux

```bash
rm -rf /tmp/aliasmgr-smoke
```

### Windows PowerShell

```powershell
Remove-Item -Recurse -Force "$env:TEMP\aliasmgr-smoke"
```

保留 `result/` 目录中的输出文件，直到完成反馈提交。
