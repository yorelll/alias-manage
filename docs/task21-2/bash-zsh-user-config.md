# Task 21-2 Bash/Zsh 用户配置人工验收

使用专用临时用户、可信 dotfiles checkout 或一次性配置目录。重点检查真实 RC 顺序和用户-owned 配置边界，这些内容 CI 无法替代。每个案例记录 Shell 版本、脱敏 RC 路径、symlink 分类、artifact SHA256、实际结果、结果枚举、证据、复现说明和脱敏确认。

## U-001：标记 loader 追加和幂等

1. 使用脱敏 hash/行数保存无关 RC 内容快照。
2. Bash 和 Zsh 分别安装 loader 两次。
3. 确认只有一个完整 marker pair 且位于末尾。

预期：标记块只出现一次；无关内容和行尾不变。

## U-002：login 与非交互链

1. 启动交互和非交互 Bash。
2. 启动 login Bash，检查是否 source `.bashrc`，不要上传完整文件。
3. 对 Zsh 重复检查 `.zprofile`/`.zshrc`。

预期：交互限制符合文档；缺失 login chain 只提示人工操作，不自动修改。

## U-003：oh-my-zsh 后置覆盖

1. 在隔离环境使用 oh-my-zsh（如环境允许）。
2. 在 loader 后加载的插件中创建临时同名 alias。
3. 运行 Doctor 或检查来源顺序，只记录文件和行号摘要。

预期：后置覆盖被报告；不自动竞争或重排用户插件。

## U-004：RC symlink 和行尾

1. 将 `.bashrc` 或 `.zshrc` 指向可信用户-owned target。
2. 安装和移除 loader。
3. 检查 symlink identity 和 target hash。

预期：可信 symlink 保留且 target 就地修改；跨用户/全局可写链路阻止或要求明确分类。

## U-005：手工修改和 checksum

1. 同步临时生成文件。
2. 手工做无害修改。
3. 再次同步并观察备份和决策提示。

预期：产生备份；不静默丢弃用户修改；记录用户决策和证据。

## U-006：实时 tombstone/fingerprint

1. 在同一交互会话创建并加载别名。
2. 在应用中删除、禁用或改名。
3. 在同一会话 reload。
4. 重建同名用户定义后再次 reload。

预期：托管定义清除；用户重建定义保留并报告 skip；无法可靠获取 host fingerprint 时标记 `EXPECTED-LIMITATION` 或 `BLOCKED`。

## U-007：覆盖恢复

1. 创建临时用户 alias/function。
2. 进行明确确认的托管覆盖。
3. 只检查脱敏 override metadata 和 recoverability。
4. 分别测试可解析和不可解析定义。

预期：可解析定义为可恢复；不可解析定义为不可自动恢复；原始私有内容不进入报告。
