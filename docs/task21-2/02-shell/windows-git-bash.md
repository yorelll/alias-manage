# Windows Git Bash 探索性验收手册

**分类：探索性 / 有限制**

Windows Git Bash **不能**替代 Linux Bash 验证。本手册的所有案例均为探索性记录，结果应标记为 `EXPECTED-LIMITATION` 或 `BLOCKED`（视情况而定）。

---

## 为什么 Git Bash 是探索性环境

Git Bash 运行在 MinGW/MSYS2 之上，提供类 Unix Shell 体验，但存在以下固有限制：

1. **它不是 Linux Bash**：进程模型、文件系统路径转换（`/c/Users/...` vs `C:\Users\...`）、信号处理等均与原生 Linux Bash 不同。
2. **无法替代 Linux 原生行为验证**：login chain、`/etc/profile` 行为、Linux 权限模型均无法在 Git Bash 中准确重现。
3. **loader 行为可能不可预期**：RC 文件路径解析在 Git Bash 下可能产生与 Linux 不同的结果。

---

## 探索性记录范围

在 Git Bash 中，以下内容可以探索性记录（结果不纳入正式验收矩阵，但可附作参考信息）：

| 探索项 | 说明 |
|---|---|
| CLI artifact 可执行性 | Windows artifact（.exe）是否能在 Git Bash 中以 `./aliasmgr.exe` 方式运行 |
| `--version` 输出 | 版本命令是否返回正常输出 |
| 基础 list/add | 简单 CRUD 是否可用（不涉及 RC 文件修改） |
| 路径转换副作用 | 观察 Git Bash 路径转换是否导致配置目录路径异常 |

---

## 探索步骤

### 打开 Git Bash 终端

1. 点击开始菜单，搜索 **Git Bash**，打开终端。
2. 确认当前 Shell：

   ```bash
   echo $SHELL
   bash --version
   uname -a
   ```

   注意：`uname -a` 会显示 MinGW 或 MSYS2 信息，而不是 Linux。

### 基础 CLI 可用性探索

```bash
# 准备临时目录
GITBASH_SMOKE="$TEMP/aliasmgr-gitbash"
mkdir -p "$GITBASH_SMOKE/config" "$GITBASH_SMOKE/result"

# 尝试运行（路径按 Git Bash 方式转换）
/c/Users/$USERNAME/AppData/Local/Temp/aliasmgr-smoke/aliasmgr.exe --version

# 记录版本输出
/c/Users/$USERNAME/AppData/Local/Temp/aliasmgr-smoke/aliasmgr.exe --version \
  > "$GITBASH_SMOKE/result/version.txt" 2>&1

# 观察路径转换
echo "TEMP in Git Bash: $TEMP"
echo "Expected Windows path: C:\Users\...\AppData\Local\Temp"
```

### 观察和记录

记录以下信息（用于参考，不用于正式矩阵）：

- Git Bash 版本和 MSYS2/MinGW 版本
- CLI artifact 是否可运行
- 路径转换是否产生异常错误
- loader 安装是否被阻止（预期：可能因路径转换而行为异常）

---

## 结果分类指引

| 观察结果 | 建议结果 | 原因 |
|---|---|---|
| CLI 可运行，CRUD 可用 | `EXPECTED-LIMITATION` | Git Bash 不是支持的验收 Shell |
| loader 安装失败 | `EXPECTED-LIMITATION` | MinGW 路径转换导致 RC 文件处理异常，属文档化限制 |
| loader 安装成功但行为与 Linux 不同 | `EXPECTED-LIMITATION` | 探索性环境差异 |
| CLI 无法运行（EXE 格式问题除外） | `BLOCKED` | 记录阻塞原因 |

**不要**将 Git Bash 的 `PASS` 结果作为 Linux Bash 案例（L-009、L-011 等）的证据。

---

## 清理

```bash
rm -rf "$TEMP/aliasmgr-gitbash"
```
