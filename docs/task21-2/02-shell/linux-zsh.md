# Linux Zsh Shell 集成验证手册

涵盖案例：L-010、L-011（Zsh 部分）、L-012

在开始之前，请先完成 [../01-cli/linux-cli.md](../01-cli/linux-cli.md) 中的 CLI 冒烟测试。

---

## 准备工作

### 打开 Zsh 终端

1. 打开系统终端并切换到 Zsh：

   ```bash
   zsh --version
   ```

2. 记录 Zsh 版本（例如 `zsh 5.9 (x86_64-pc-linux-gnu)`）。

3. 确认测试中只使用临时 RC 文件，不触碰 `~/.zshrc` 或 `~/.zprofile`。

### 准备临时目录

```zsh
ZSH_SMOKE=$HOME/.zsh-smoke-test
ZSH_CFG=$ZSH_SMOKE/config
ZSH_RC=$ZSH_SMOKE/test-zshrc
ZSH_RESULT=$ZSH_SMOKE/result
ZSH_GEN=$ZSH_SMOKE/generated

mkdir -p $ZSH_CFG $ZSH_RESULT $ZSH_GEN
touch $ZSH_RC
```

---

## L-010：Zsh loader/reload

### 步骤

1. 添加测试别名：

   ```zsh
   aliasmgr add \
     --config-dir $ZSH_CFG \
     --name zsh-smoke-alias \
     --target "echo zsh-smoke-ok" \
     --shell zsh
   ```

2. 安装 loader 到临时 RC 文件（第一次）：

   ```zsh
   aliasmgr loader install \
     --config-dir $ZSH_CFG \
     --rc-file $ZSH_RC \
     --generated-dir $ZSH_GEN
   ```

3. 确认 marker 块只出现一次：

   ```zsh
   cat $ZSH_RC
   ```

4. 再次安装 loader（测试幂等性）：

   ```zsh
   aliasmgr loader install \
     --config-dir $ZSH_CFG \
     --rc-file $ZSH_RC \
     --generated-dir $ZSH_GEN
   ```

5. 确认仍只有一个 marker 块：

   ```zsh
   grep -c "aliasmgr" $ZSH_RC
   ```

6. 同步生成文件：

   ```zsh
   aliasmgr sync --config-dir $ZSH_CFG --generated-dir $ZSH_GEN
   ls $ZSH_GEN
   ```

7. 启动隔离子 Shell（加载临时 RC），验证别名：

   ```zsh
   zsh -i -c "source $ZSH_RC; type zsh-smoke-alias"
   ```

8. 移除 loader，确认 marker 块消失：

   ```zsh
   aliasmgr loader remove \
     --config-dir $ZSH_CFG \
     --rc-file $ZSH_RC
   cat $ZSH_RC
   ```

### 预期

- Marker 块只出现一次；生成别名正确加载；无关内容不变；移除后 marker 块消失。

---

## L-011：Zsh login/非交互链（手动跟进案例）

### 步骤

1. 启动交互非登录 Zsh：

   ```zsh
   zsh -i -c "source $ZSH_RC; type zsh-smoke-alias && echo '交互加载: OK'"
   ```

2. 启动非交互 Zsh（期望别名不加载）：

   ```zsh
   zsh -c "type zsh-smoke-alias 2>&1 || echo '非交互: 不加载 (预期)'"
   ```

3. 检查 `.zprofile` → `.zshrc` login 链（只记录观察摘要，不上传完整文件）：

   ```zsh
   zsh -l -c "echo 'login Zsh: 请手动检查 .zprofile 是否 source .zshrc'"
   ```

### 预期

- 交互非登录加载 RC；非交互不加载；缺失 login chain 只提示手动操作。

---

## L-012：oh-my-zsh 顺序（手动跟进案例，环境允许时执行）

如果当前环境没有 oh-my-zsh，标记 `NOT-APPLICABLE` 并说明原因。

### 步骤

1. 在隔离 Zsh 环境（非真实 oh-my-zsh 安装），创建临时模拟后置同名 alias：

   ```zsh
   # 在 ZSH_RC 末尾（loader 之后）添加模拟后置覆盖
   echo "alias zsh-smoke-alias='echo omz-overridden'" >> $ZSH_RC
   ```

2. 启动隔离 Zsh，检查哪个来源生效：

   ```zsh
   zsh -i -c "source $ZSH_RC; type zsh-smoke-alias; zsh-smoke-alias"
   ```

3. 运行 Doctor 或来源检查，只记录文件名和行号摘要（不上传 RC 内容）：

   ```zsh
   aliasmgr doctor --config-dir $ZSH_CFG --rc-file $ZSH_RC
   ```

4. 确认：后置覆盖被报告；不自动竞争或重排用户插件。

### 预期

- 后置覆盖被检测和报告；Doctor 不自动修改用户 RC 顺序。

---

## Zsh 特有边缘案例

### 行尾和 syntax 检查

```zsh
# 检查生成文件语法
zsh -n $ZSH_GEN/aliases.zsh && echo "syntax OK"
```

### 特殊参数和 setopt 兼容性

如果系统 Zsh 使用了影响 alias 展开的 setopt（如 `ALIASES`、`GLOBAL_ALIASES`），记录配置摘要，不上传完整 zshenv。

---

## 保存结果日志

```zsh
echo "Linux Zsh Shell smoke log" > $ZSH_RESULT/verification.log
echo "Timestamp: $(date -u +%Y-%m-%dT%H:%M:%SZ)" >> $ZSH_RESULT/verification.log
echo "Zsh version: $(zsh --version)" >> $ZSH_RESULT/verification.log
echo "Artifact SHA256: [从 SHA256SUMS 文件填入]" >> $ZSH_RESULT/verification.log
```

---

## 清理

```zsh
rm -rf $ZSH_SMOKE
```
