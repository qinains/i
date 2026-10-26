# 文档中心

先选使用方式，再进入对应契约。浏览器 WASM 插件、原生 worker、浏览器 CORS 扩展
是三个不同的东西，不能互相安装或混用包格式。

## 开始使用

- [第一次连接 AI](connect-model.md)：到哪里申请 API Key，URL 和模型怎么填，失败时怎么查。
- [系统与模块概览](system-overview.md)：组件、数据流、文件与会话边界。
- [浏览器工作台](wasm.md)：构建、模型连接、PWA、预览、文件与恢复。
- [桌面安装与配置](deployment.md)：源码、发布包、目录与凭据。
- [控制台](console.md)：命令、粘贴、交互与会话。
- [文件工具](file-tools.md)：减少调用轮次的查找、批量读与精确编辑。

## 为系统开发插件

- [从零编写插件](plugin-development.md)：两个可复现的 Rust 教程。
- [WASM 插件参考](wasm-plugin-reference.md)：清单、ABI、host API、限制。
- [原生 worker 参考](plugins.md)：NDJSON、hooks、工具和路径上下文。
- [Provider 插件](providers.md)：模型协议、传输、重试与鉴权边界。
- [审批策略](approval.md)、[预算与压缩](compaction.md)、[缓存统计](cache.md)。

## 运维、边界与贡献

- [OPFS 并发与持久化边界](opfs-concurrency.md)
- [CORS 扩展安装与限制](cors-extension.md)
- [官网与 Caddy 部署](site-deployment.md)
- [贡献与 PR](../CONTRIBUTING.md)
- [安全报告](../SECURITY.md)
- [MIT 许可](../LICENSE)
