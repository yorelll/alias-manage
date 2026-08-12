# Testing

Rust workspace、Shell 适配器和跨平台测试由 GitHub Actions 执行。本地环境没有 Rust 工具链，因此不运行 Cargo 命令。CI 测试必须使用临时 HOME、配置目录和 Profile，不能修改 runner 真实用户配置。

GUI 的视觉和真实终端行为仍需下载构建产物后人工确认。
