# Linux 安装包验收手册

涵盖案例：PKG-001（Linux 部分）、PKG-002（Linux 部分）、PKG-003（Linux 部分）、PKG-004（Linux 部分）

**只有在真实 Linux 安装包 artifact（AppImage / .deb / .rpm）存在时执行。**

如果没有安装包 artifact，在矩阵中标记 `NOT-APPLICABLE` 并写明具体缺失的 artifact 类型和原因。

---

## 缺少 artifact 的处理指引

| 情况 | 矩阵结果 | 说明示例 |
|---|---|---|
| AppImage 未在 CI 中生成 | `NOT-APPLICABLE` | "AppImage artifact 在此 prerelease 阶段未提供" |
| .deb 包生成失败 | `BLOCKED` | "CI deb 打包 job 因 tauri 依赖失败，无法执行 .deb 验收" |
| 平台不支持 .rpm | `NOT-APPLICABLE` | "当前测试环境为 Ubuntu，.rpm 不适用" |

---

## 准备工作

1. 确认使用**一次性 Linux 用户**或独立 VM，不要在开发机主账户上安装候选包。

2. 校验 artifact SHA256：

   ```bash
   sha256sum -c SHA256SUMS
   ```

3. 保存无关配置的前后快照：

   ```bash
   ls ~/.config/aliasmgr 2>/dev/null > /tmp/before-install-snapshot.txt
   cat ~/.bashrc | wc -l >> /tmp/before-install-snapshot.txt
   ```

---

## PKG-001：干净安装包安装

### AppImage 步骤

1. 添加执行权限：

   ```bash
   chmod +x ./aliasmgr-linux-x86_64.AppImage
   ```

2. 运行 AppImage（不安装到系统）：

   ```bash
   ./aliasmgr-linux-x86_64.AppImage --version
   ```

   如果 FUSE 不可用，使用解压模式：

   ```bash
   ./aliasmgr-linux-x86_64.AppImage --appimage-extract-and-run --version
   ```

3. 记录版本字符串。

4. 确认无关配置未被修改：

   ```bash
   ls ~/.config/aliasmgr 2>/dev/null >> /tmp/after-install-snapshot.txt
   diff /tmp/before-install-snapshot.txt /tmp/after-install-snapshot.txt
   ```

### .deb 步骤（如适用）

1. 安装：

   ```bash
   sudo dpkg -i ./aliasmgr_X.Y.Z_amd64.deb
   ```

2. 如果有依赖缺失：

   ```bash
   sudo apt-get install -f
   ```

3. 验证安装：

   ```bash
   aliasmgr --version
   which aliasmgr
   ```

4. 检查安装是否修改了无关配置：

   ```bash
   diff /tmp/before-install-snapshot.txt <(ls ~/.config/aliasmgr 2>/dev/null)
   ```

### 预期

- 安装不修改无关配置；应用启动并报告正确版本。

---

## PKG-002：升级和回滚

### 步骤

1. 先安装旧候选版本（如果有）并创建临时别名：

   ```bash
   aliasmgr add --name pkg-upgrade-linux --target "echo pre-upgrade" --shell bash
   ```

2. 安装当前候选版本（升级）：

   ```bash
   # AppImage：直接替换文件
   # .deb：sudo dpkg -i new-version.deb
   ```

3. 验证升级后数据保留：

   ```bash
   aliasmgr --version
   aliasmgr list
   aliasmgr get pkg-upgrade-linux
   ```

4. 检查备份目录：

   ```bash
   ls ~/.config/aliasmgr/backup* 2>/dev/null || echo "无备份文件"
   ```

5. 如果支持回滚（记录是否支持），执行并重复验证。

### 预期

- 升级保留配置和别名；目标文件保留；回滚恢复文档化状态。

---

## PKG-003：postrm/MSI hook

### 步骤

1. 准备引用临时目标的别名：

   ```bash
   echo "# pkg hook test target" > /tmp/pkg-hook-target.sh
   aliasmgr add --name pkg-hook-linux --target "bash /tmp/pkg-hook-target.sh" --shell bash
   ```

2. 通过包管理器卸载（.deb 示例）：

   ```bash
   # retain 模式
   sudo dpkg --remove aliasmgr
   # 或 purge 模式（如测试 purge hook）
   # sudo dpkg --purge aliasmgr
   ```

   对于 AppImage（无系统 hook），记录 AppImage 无 postrm hook，标记 `NOT-APPLICABLE`。

3. 验证 hook 行为：
   - 确认卸载过程非交互（无等待用户输入的提示）。
   - 确认目标文件未被删除：`ls /tmp/pkg-hook-target.sh`
   - 确认无关 Profile 行未被删除。

### 预期

- postrm hook 非交互；不删除目标文件；不删除无关 Profile 行；引导用户显式执行清理。

### AppImage 说明

AppImage 没有 postrm hook，PKG-003 对 AppImage 标记 `NOT-APPLICABLE`，说明"AppImage 无系统级 postrm hook，此案例不适用"。

---

## PKG-004：非托管内容

### 步骤

1. 在无关 Profile 和配置目录放置 sentinel 内容（安装前）：

   ```bash
   echo "# sentinel - should not be modified" >> /tmp/test-sentinel-bashrc
   cp /tmp/test-sentinel-bashrc /tmp/test-sentinel-bashrc.before
   ```

2. 分别执行安装、升级、卸载（每步后比较）：

   ```bash
   sha256sum /tmp/test-sentinel-bashrc
   diff /tmp/test-sentinel-bashrc.before /tmp/test-sentinel-bashrc
   ```

3. 执行 purge 卸载（如支持），再次比较。

### 预期

- 非托管内容在所有操作后保持不变；任何文档化规范化必须明确报告。

---

## 保存结果日志

```bash
{
  echo "Linux package smoke log"
  echo "Timestamp: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "Artifact SHA256: [从 SHA256SUMS 文件填入]"
  echo "OS: $(uname -srm)"
  echo "Distro: $(lsb_release -d 2>/dev/null || cat /etc/os-release | head -3)"
} > /tmp/aliasmgr-pkg-result.log
```

---

## 清理

```bash
rm -f /tmp/before-install-snapshot.txt \
      /tmp/after-install-snapshot.txt \
      /tmp/pkg-hook-target.sh \
      /tmp/test-sentinel-bashrc \
      /tmp/test-sentinel-bashrc.before \
      /tmp/aliasmgr-pkg-result.log
```
