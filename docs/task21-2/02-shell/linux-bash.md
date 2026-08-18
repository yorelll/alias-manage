# Linux Bash Shell 集成验证手册

涵盖案例：L-009、L-011（Bash 部分）、L-013、L-014、L-015、L-016

在开始之前，请先完成 [../01-cli/linux-cli.md](../01-cli/linux-cli.md) 中的 CLI 冒烟测试。

---

## 准备工作

### 打开 Bash 终端

1. 打开系统终端。
2. 确认当前 Shell 是 Bash：

   ```bash
   echo $SHELL
   bash --version
   ```

3. 记录 Bash 版本。

### 准备临时目录

```bash
export BASH_SMOKE=$HOME/.bash-smoke-test
export BASH_CFG=$BASH_SMOKE/config
export BASH_RC=$BASH_SMOKE/test-bashrc
export BASH_RESULT=$BASH_SMOKE/result
export BASH_GEN=$BASH_SMOKE/generated

mkdir -p $BASH_CFG $BASH_RESULT $BASH_GEN
touch $BASH_RC
```

**重要：** 整个测试过程中，只修改 `$BASH_RC`（临时 RC 文件），绝不修改真实的 `~/.bashrc`。

---

## L-009：Bash loader/reload

### 步骤

1. 先添加一个测试别名（使用临时配置目录）：

   ```bash
   aliasmgr add \
     --config-dir $BASH_CFG \
     --name bash-smoke-alias \
     --target "echo bash-smoke-ok" \
     --shell bash
   ```

2. 安装 loader 到临时 RC 文件（第一次）：

   ```bash
   aliasmgr loader install \
     --config-dir $BASH_CFG \
     --rc-file $BASH_RC \
     --generated-dir $BASH_GEN
   ```

3. 查看临时 RC 文件，确认 marker 块只出现一次：

   ```bash
   cat $BASH_RC
   ```

4. 再次安装 loader（测试幂等性）：

   ```bash
   aliasmgr loader install \
     --config-dir $BASH_CFG \
     --rc-file $BASH_RC \
     --generated-dir $BASH_GEN
   ```

5. 再次查看，确认 marker 块仍只有一个：

   ```bash
   grep -c "aliasmgr" $BASH_RC
   ```

6. 同步生成别名文件：

   ```bash
   aliasmgr sync --config-dir $BASH_CFG --generated-dir $BASH_GEN
   ls $BASH_GEN
   ```

7. 启动隔离的子 Shell，source 临时 RC 并验证别名：

   ```bash
   bash --rcfile $BASH_RC -i <<'EOF'
   type bash-smoke-alias
   EOF
   ```

8. 移除 loader：

   ```bash
   aliasmgr loader remove \
     --config-dir $BASH_CFG \
     --rc-file $BASH_RC
   ```

9. 确认 marker 块已移除，无关内容保留：

   ```bash
   cat $BASH_RC
   ```

### 预期

- Marker 块只出现一次；生成别名正确加载；无关内容和行尾不变；移除后 marker 块消失。

---

## L-011：Bash login/非交互链（手动跟进案例）

此案例需要手动执行，自动化脚本无法替代。

### 步骤

1. 启动交互非登录 Bash，确认别名加载：

   ```bash
   bash --rcfile $BASH_RC -i <<'EOF'
   type bash-smoke-alias && echo "交互加载: OK"
   EOF
   ```

2. 启动非交互 Bash（期望别名不加载）：

   ```bash
   bash --norc -c "type bash-smoke-alias 2>&1 || echo '非交互: 不加载 (预期)'"
   ```

3. 启动 login Bash，手动检查是否 source `.bashrc`（不上传完整文件）：

   ```bash
   bash -l -c "echo 'login chain test; 请手动检查 /etc/profile 是否 source ~/.bashrc'"
   ```

   记录观察结果：是否有 login chain；如果没有，记录 `EXPECTED-LIMITATION`。

### 预期

- 交互非登录 Shell 加载 RC；非交互不加载；缺失 login chain 只提示手动操作，不自动修改。

---

## L-013：RC symlink 和行尾

### 步骤

1. 创建可信 symlink 目标（用户 owned）：

   ```bash
   touch $BASH_SMOKE/symlink-target.bashrc
   ln -s $BASH_SMOKE/symlink-target.bashrc $BASH_SMOKE/test-rc-symlink
   ```

2. 对 symlink RC 安装 loader：

   ```bash
   aliasmgr loader install \
     --config-dir $BASH_CFG \
     --rc-file $BASH_SMOKE/test-rc-symlink \
     --generated-dir $BASH_GEN
   ```

3. 确认 symlink identity 保留（symlink 本身不被替换为普通文件）：

   ```bash
   ls -la $BASH_SMOKE/test-rc-symlink
   file $BASH_SMOKE/test-rc-symlink
   ```

4. 检查行尾摘要（只记录摘要，不上传文件内容）：

   ```bash
   file $BASH_SMOKE/symlink-target.bashrc
   ```

### 预期

- 可信 symlink 保留；target 就地修改；行尾摘要符合预期。

---

## L-014：手工修改生成文件

### 步骤

1. 同步后手工修改生成文件：

   ```bash
   echo "# user modification" >> $BASH_GEN/aliases.bash
   ```

2. 再次同步：

   ```bash
   aliasmgr sync --config-dir $BASH_CFG --generated-dir $BASH_GEN
   ```

3. 确认备份已创建，并有决策提示：

   ```bash
   ls $BASH_GEN
   ```

### 预期

- 产生备份；不静默丢弃用户修改；记录用户决策选项。

---

## L-015/L-016：tombstone 和恢复

### 步骤

1. 创建并加载别名：

   ```bash
   aliasmgr add --config-dir $BASH_CFG --name tomb-test --target "echo tomb-ok" --shell bash
   aliasmgr sync --config-dir $BASH_CFG --generated-dir $BASH_GEN
   ```

2. 禁用别名：

   ```bash
   aliasmgr disable --config-dir $BASH_CFG tomb-test
   aliasmgr sync --config-dir $BASH_CFG --generated-dir $BASH_GEN
   ```

3. 在隔离子 Shell 确认 reload 后别名不再可用：

   ```bash
   bash --rcfile $BASH_RC -i <<'EOF'
   type tomb-test 2>&1 || echo "已禁用: OK"
   EOF
   ```

4. 删除别名并再次 sync/reload：

   ```bash
   aliasmgr delete --config-dir $BASH_CFG tomb-test
   aliasmgr sync --config-dir $BASH_CFG --generated-dir $BASH_GEN
   ```

5. 创建同名用户 alias，确认 reload 时不被静默删除：

   ```bash
   echo "alias tomb-test='echo user-defined'" >> $BASH_RC
   bash --rcfile $BASH_RC -i <<'EOF'
   type tomb-test
   EOF
   ```

### 预期

- 只清除托管残留；用户定义 alias 保留并报告 skip；不声称子进程能更新父会话。

---

## 保存结果日志

```bash
echo "Linux Bash Shell smoke log" > $BASH_RESULT/verification.log
echo "Timestamp: $(date -u +%Y-%m-%dT%H:%M:%SZ)" >> $BASH_RESULT/verification.log
echo "Bash version: $(bash --version | head -1)" >> $BASH_RESULT/verification.log
echo "Artifact SHA256: [从 SHA256SUMS 文件填入]" >> $BASH_RESULT/verification.log
```

---

## 清理

```bash
rm -rf $BASH_SMOKE
```
