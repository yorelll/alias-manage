# Linux CLI 冒烟测试手册

涵盖案例：L-001 至 L-008、L-017、L-018、L-019

在开始之前，请先完成 [artifact-smoke.md](artifact-smoke.md) 中的下载和校验步骤。

---

## 准备工作

### 打开终端

1. 打开系统终端（GNOME Terminal、Konsole、xterm 或 SSH 会话均可）。
2. 确认当前用户是专用测试用户或一次性 VM 用户，**不要使用主账户的生产配置**。

### 准备测试目录

```bash
export SMOKE_ROOT=/tmp/aliasmgr-l-smoke
export SMOKE_BIN=$SMOKE_ROOT/bin
export SMOKE_CFG=$SMOKE_ROOT/config
export SMOKE_RESULT=$SMOKE_ROOT/result
export SMOKE_TARGET=$SMOKE_ROOT/targets

mkdir -p $SMOKE_BIN $SMOKE_CFG $SMOKE_RESULT $SMOKE_TARGET
```

### 将 artifact 复制到测试目录

```bash
cp /tmp/aliasmgr-smoke/aliasmgr $SMOKE_BIN/aliasmgr
chmod +x $SMOKE_BIN/aliasmgr
```

---

## L-001：干净安装/启动/版本

### 步骤

1. 运行版本检查命令：

   ```bash
   $SMOKE_BIN/aliasmgr --version
   ```

2. 记录输出的版本字符串和 SHA256（从 `SHA256SUMS` 文件获取）。

3. 确认临时配置目录为空：

   ```bash
   ls $SMOKE_CFG
   ```

### 预期

- 命令输出版本号，格式为 `aliasmgr X.Y.Z` 或类似。
- 无关的 `~/.config/aliasmgr` 或其他配置目录未被修改。

---

## L-002：添加原生别名

### 步骤

1. 添加一个测试别名：

   ```bash
   $SMOKE_BIN/aliasmgr add \
     --config-dir $SMOKE_CFG \
     --name smoke-test-native \
     --target "echo hello-world" \
     --shell bash \
     --description "冒烟测试：原生别名"
   ```

2. 列出全部别名：

   ```bash
   $SMOKE_BIN/aliasmgr list --config-dir $SMOKE_CFG
   ```

3. 精确获取该别名：

   ```bash
   $SMOKE_BIN/aliasmgr get --config-dir $SMOKE_CFG smoke-test-native
   ```

4. 重启（退出并重新打开终端，或另起一个终端），再次运行步骤 2 和 3。

### 预期

- 别名名称、目标、Shell、enabled 状态和描述持久化。
- 重启后数据仍存在。

---

## L-003：Python/JAR/切换目录目标类型

### 步骤

1. 创建无害 Python 脚本目标：

   ```bash
   cat > $SMOKE_TARGET/test-python.py << 'EOF'
   import sys
   print("argv:", sys.argv[1:])
   EOF

   $SMOKE_BIN/aliasmgr add \
     --config-dir $SMOKE_CFG \
     --name smoke-test-python \
     --target "python3 $SMOKE_TARGET/test-python.py" \
     --shell bash
   ```

2. 创建切换目录目标（不执行）：

   ```bash
   $SMOKE_BIN/aliasmgr add \
     --config-dir $SMOKE_CFG \
     --name smoke-test-cd \
     --target "$SMOKE_TARGET" \
     --type change-directory \
     --shell bash
   ```

3. 检查每个别名的类型和目标预览（不实际执行切换目录）：

   ```bash
   $SMOKE_BIN/aliasmgr get --config-dir $SMOKE_CFG smoke-test-python
   $SMOKE_BIN/aliasmgr get --config-dir $SMOKE_CFG smoke-test-cd
   ```

4. 如果没有 JDK，跳过 JAR 测试并在矩阵中标记 `NOT-APPLICABLE`，说明无 JDK。

### 预期

- 目标类型校验和渲染正确；切换目录只影响当前 Shell，不执行系统命令。

---

## L-004：argv 精确边界

### 步骤

1. 创建 argument dumper 脚本：

   ```bash
   cat > $SMOKE_TARGET/argv-dump.sh << 'EOF'
   #!/usr/bin/env bash
   for i in "$@"; do
     printf '%s\n' "$i"
   done
   EOF
   chmod +x $SMOKE_TARGET/argv-dump.sh
   ```

2. 添加 argv 测试别名：

   ```bash
   $SMOKE_BIN/aliasmgr add \
     --config-dir $SMOKE_CFG \
     --name smoke-argv \
     --target "bash $SMOKE_TARGET/argv-dump.sh" \
     --shell bash
   ```

3. 在预览模式下测试各种参数（不实际运行目标）：

   ```bash
   $SMOKE_BIN/aliasmgr preview --config-dir $SMOKE_CFG smoke-argv -- "" "hello world" "它好" 'single"quote' $'\t'
   ```

4. 将预览输出中每个参数与输入逐一比对，记录到 `$SMOKE_RESULT/argv-summary.json`。

### 预期

- 空元素和所有边界字符保留；无通配符展开、拼接或秘密输出。

---

## L-005：`{{args}}` 矩阵

### 步骤

1. 测试末尾隐式透传（添加带 `{{args}}` 的别名）：

   ```bash
   $SMOKE_BIN/aliasmgr add \
     --config-dir $SMOKE_CFG \
     --name smoke-args-end \
     --target "bash $SMOKE_TARGET/argv-dump.sh {{args}}" \
     --shell bash
   ```

2. 测试中间插入：

   ```bash
   $SMOKE_BIN/aliasmgr add \
     --config-dir $SMOKE_CFG \
     --name smoke-args-mid \
     --target "bash $SMOKE_TARGET/argv-dump.sh fixed1 {{args}} fixed2" \
     --shell bash
   ```

3. 尝试添加非法形式（期望被拒绝）：

   ```bash
   $SMOKE_BIN/aliasmgr add \
     --config-dir $SMOKE_CFG \
     --name smoke-args-bad \
     --target "bash $SMOKE_TARGET/argv-dump.sh {{args}} {{args}}" \
     --shell bash
   ```

4. 记录每种形式的接受/拒绝状态。

### 预期

- 合法形式保存成功；非法形式（如重复 `{{args}}`）被拒绝且不保存。

---

## L-006：工作目录/环境/标签

### 步骤

1. 添加带工作目录和标签的别名：

   ```bash
   $SMOKE_BIN/aliasmgr add \
     --config-dir $SMOKE_CFG \
     --name smoke-cwd-tagged \
     --target "echo cwd-test" \
     --cwd "$SMOKE_TARGET" \
     --tag "smoke" --tag "cwd" \
     --shell bash
   ```

2. 查看详情，确认工作目录和标签持久化：

   ```bash
   $SMOKE_BIN/aliasmgr get --config-dir $SMOKE_CFG smoke-cwd-tagged
   ```

3. 环境变量测试：只使用无害标记名，不使用密码、Token 等敏感值。

### 预期

- 工作目录和标签持久化；敏感值不进入日志或导出。

---

## L-007/L-008：搜索和标签分面

### 步骤

1. 确认已有若干不同名称、描述和标签的别名（之前步骤创建）。

2. 测试文本搜索：

   ```bash
   $SMOKE_BIN/aliasmgr list --config-dir $SMOKE_CFG --filter "smoke"
   ```

3. 测试标签单选：

   ```bash
   $SMOKE_BIN/aliasmgr list --config-dir $SMOKE_CFG --tag "cwd"
   ```

4. 测试多标签 AND：

   ```bash
   $SMOKE_BIN/aliasmgr list --config-dir $SMOKE_CFG --tag "smoke" --tag "cwd"
   ```

5. 清除筛选，列出全部：

   ```bash
   $SMOKE_BIN/aliasmgr list --config-dir $SMOKE_CFG
   ```

### 预期

- 多标签为 AND 关系；清除筛选后恢复全部结果。

---

## L-017：JSON/TOML 导入

### 步骤

1. 导出当前配置：

   ```bash
   $SMOKE_BIN/aliasmgr export --config-dir $SMOKE_CFG --format json > $SMOKE_RESULT/export.json
   ```

2. 预览导入（不实际导入）：

   ```bash
   $SMOKE_BIN/aliasmgr import --config-dir $SMOKE_CFG --preview $SMOKE_RESULT/export.json
   ```

   确认预览只读，不产生数据库写入。

3. 实际导入：

   ```bash
   $SMOKE_BIN/aliasmgr import --config-dir $SMOKE_CFG $SMOKE_RESULT/export.json
   ```

4. 验证持久化：

   ```bash
   $SMOKE_BIN/aliasmgr list --config-dir $SMOKE_CFG
   ```

### 预期

- 预览只读；确认后持久化；importated/skipped/warnings 有明确报告。

---

## L-018/L-019：卸载、升级和回滚

### 步骤

1. 记录当前配置目录状态：

   ```bash
   ls $SMOKE_CFG
   ```

2. 测试 retain 卸载（保留数据）：

   ```bash
   $SMOKE_BIN/aliasmgr uninstall --config-dir $SMOKE_CFG --mode retain
   ```

   验证目标文件未被删除：

   ```bash
   ls $SMOKE_TARGET
   ```

3. 如果提供了升级候选，将新版本 artifact 复制到 `$SMOKE_BIN` 并重复 L-001 版本检查，然后检查配置保留情况。

### 预期

- retain 模式只移除托管元数据；目标文件保留；无关内容保留。

---

## 保存结果日志

```bash
echo "L-001 through L-019 smoke log" > $SMOKE_RESULT/verification.log
echo "Timestamp: $(date -u +%Y-%m-%dT%H:%M:%SZ)" >> $SMOKE_RESULT/verification.log
echo "Artifact SHA256: [从 SHA256SUMS 文件填入]" >> $SMOKE_RESULT/verification.log
echo "OS: $(uname -srm)" >> $SMOKE_RESULT/verification.log
```

将 `$SMOKE_RESULT/` 目录中的文件作为证据附件提交，提交前确认无用户名、完整路径、密码或 Token。

---

## 清理

```bash
rm -rf $SMOKE_ROOT
```
