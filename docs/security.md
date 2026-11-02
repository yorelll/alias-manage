# Security

普通生成路径使用参数数组和目标 Shell 的字面量转义，不使用 `eval` 或 `Invoke-Expression`。高级 Shell 模式和 RawShellCommand 在 MVP 校验阶段拒绝。

限行（执行目录限制）会生成目录守卫代码，但它只是防护栏而不是安全沙箱：只检查调用时的当前目录，直接按路径调用或修改生成文件都可绕过。它不能替代文件权限、容器或操作系统级访问控制。详见 [restrictions.md](restrictions.md)。

配置文件只包含 Alias Manager 标记加载块；目标文件不会由卸载流程删除。导入配置必须经过预览和安全检查。敏感环境变量不得写入日志或导出。
