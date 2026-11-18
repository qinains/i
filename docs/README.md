# 项目文档索引

**最后更新**: 2026-11-13

---

## 📚 核心文档

### 架构与设计

- [系统概览](system-overview.md) - 整体架构和组件说明
- [Cordis 架构](CORDIS_ARCHITECTURE.md) - 插件优先的架构框架
- [网络架构](network.md) - 网络通信和协议
- [部署指南](deployment.md) - 部署和运维

### 核心功能

- [插件系统](plugins.md) - 插件开发和生命周期
- [插件开发](plugin-development.md) - 插件开发指南
- [WASM 插件](wasm.md) - WebAssembly 插件支持
- [WASM 插件参考](wasm-plugin-reference.md) - WASM 插件 API 参考
- [工作区隔离](workspace-isolation.md) - 工作区管理和隔离

### 工具和功能

- [文件工具](file-tools.md) - 文件操作工具
- [Provider 文件管理](provider-files.md) - Provider 文件管理
- [Provider 文件深入](provider-files-management.md) - Provider 文件管理详解
- [Composer](composer.md) - 组件组合系统
- [缓存系统](cache.md) - 缓存机制
- [压缩系统](compaction.md) - 数据压缩

### 浏览器和 PWA

- [浏览器 WASM](wasm.md) - 浏览器端 WASM 支持
- [OPFS 并发](opfs-concurrency.md) - Origin Private File System 并发
- [CORS 扩展](cors-extension.md) - CORS 扩展处理
- [PWA 安装](pwa-installation.md) - PWA 安装指南

---

## 🛠️ 开发文档

### 快速开始

- [贡献指南](../CONTRIBUTING.md) - 如何贡献代码
- [安全策略](../SECURITY.md) - 安全报告和政策
- [依赖审计](dependency-audit.md) - 依赖安全审计
- [依赖大小](dependency-size.md) - 依赖体积分析
- [依赖大小历史](dependency-size-history.md) - 依赖体积历史

### 高级主题

- [审批系统](approval.md) - 操作审批机制
- [国际化](i18n.md) - 多语言支持
- [站点部署](site-deployment.md) - 站点部署指南
- [打包](packaging.md) - 软件打包

---

## 📋 项目信息

### 概述文档

- [README](../README.md) - 项目说明
- [中文 README](../README.zh-CN.md) - 中文版本
- [许可证](../LICENSE) - MIT 许可证
- [更新日志](../CHANGELOG.md) - 版本历史

### 示例和配置

- [配置示例 (JSON)](../config.example.json)
- [配置示例 (TOML)](../config.example.toml)
- [配置示例 (YAML)](../config.example.yaml)
- [API 配置示例](../examples/api.anthropic.json)
- [插件配置](../examples/plugins.json)

---

## 🔧 技术文档

### 验证和测试

- [Composer 验证](tools-composer-verification.md) - Composer 验证系统
- [测试指南](../tests/) - 测试文档

### 架构决策

- [浏览器 WASM 插件设计](superpowers/specs/2026-10-23-browser-wasm-plugins-design.md)
- [浏览器 WASM 插件计划](superpowers/plans/2026-10-23-browser-wasm-plugins.md)
- [OPFS 并发计划](superpowers/plans/2026-10-23-opfs-concurrency.md)
- [OPFS 跟进计划](superpowers/plans/2026-10-23-opfs-followup.md)
- [预览功能计划](superpowers/plans/2026-10-23-scratch-preview.md)
- [CORS 扩展计划](superpowers/plans/2026-10-24-cors-extension.md)
- [公共站点发布计划](superpowers/plans/2026-10-24-public-site-release.md)

---

## 📊 任务和改进

### 架构改进

- [架构审查报告](../.tasks/ARCHITECTURE_REVIEW.md) - 架构审查和改进计划
- [P0 修复报告](../.tasks/P0_FIX_COMPLETE.md) - 关键问题修复
- [P1 完成报告](../.tasks/P1_COMPLETION_REPORT.md) - 浏览器端文档完善
- [P2 完成报告](../.tasks/P2_COMPLETION_REPORT.md) - 版本检查和错误码
- [架构改进完成报告](../.tasks/ARCHITECTURE_IMPROVEMENTS_COMPLETE.md) - 总体改进

### 架构决策记录

- [FileWatcher 改进计划](../.tasks/ADR_FILEWATCHER_IMPROVEMENT.md) - FileWatcher 改进决策

---

## 🎯 快速导航

### 新用户

1. 阅读 [README](../README.md) 了解项目
2. 查看 [系统概览](system-overview.md) 了解架构
3. 参考 [贡献指南](../CONTRIBUTING.md) 开始贡献
4. 查看 [示例配置](../config.example.json) 了解用法

### 插件开发者

1. 阅读 [插件系统](plugins.md) 了解概念
2. 参考 [插件开发](plugin-development.md) 学习开发
3. 查看 [WASM 插件](wasm.md) 了解 WASM 支持
4. 参考 [WASM 插件参考](wasm-plugin-reference.md) 了解 API

### 运维人员

1. 阅读 [部署指南](deployment.md) 了解部署
2. 查看 [站点部署](site-deployment.md) 了解站点部署
3. 参考 [PWA 安装](pwa-installation.md) 了解 PWA
4. 查看 [缓存系统](cache.md) 了解缓存配置

---

## 📝 文档维护

### 文档结构

```
docs/                    # 主要文档目录
├── README.md           # 本文件（文档索引）
├── system-overview.md  # 系统概览
├── CORDIS_ARCHITECTURE.md  # Cordis 架构
└── ...                 # 其他文档

.tasks/                 # 任务和改进文档
├── ARCHITECTURE_REVIEW.md  # 架构审查
├── P0_FIX_COMPLETE.md      # P0 修复
└── ...

根目录                  # 项目文件
├── README.md          # 项目说明
├── CONTRIBUTING.md    # 贡献指南
└── ...
```

### 文档更新指南

- 保持文档简洁明了
- 提供实际代码示例
- 及时更新过时内容
- 使用一致的格式
- 添加适当的交叉引用

---

**维护者**: Core Team
**许可证**: MIT
