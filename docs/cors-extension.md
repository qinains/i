# CORS Helper：安装、构建与边界

它是可选的浏览器扩展，不是 agent 的 `.hplugin.zip` 工具。默认关闭，采用
Manifest V3 declarativeNetRequest 修改响应头，按标签页、API 完整 URL 和请求页面
origin 限制。API key、正文与模型回复不会被扩展读取、记录或转发。

## 下载与安装

工作台：设置 → 模型连接 → 桌面浏览器 CORS 扩展。

- Chrome / Edge：下载 chromium ZIP，解压；扩展管理页开启开发者模式，加载已解压扩展。
- Firefox 142+：下载 firefox ZIP，在 `about:debugging` → 此 Firefox → 临时载入附加组件。
  未签名开发包只在当前浏览器运行期间有效；永久分发需要 Mozilla 签名。
- Safari：源码 ZIP 不能直接安装，使用 Xcode 转为容器应用后在 Safari 启用。
  仓库脚本也可以生成完整 macOS Xcode 工程 ZIP。iPhone 需要签名容器应用。

在 Harness 标签页点击扩展图标，填入网页设置显示的**完整请求 URL**，而不是只填
API 根地址。授权本页面和 API 的网站权限，再显式启用，手动重试请求。网页保持
Direct。使用后关闭；浏览器重启后默认关闭，不自动重发旧模型请求。

## 构建

```sh
python3 scripts/build_cors_extension.py
python3 scripts/build_cors_safari.py --build
node extensions/cors-helper/test-browser.mjs
```

第一个命令用标准库生成三套确定性源码 ZIP 和 SHA-256 metadata，输出
`web/downloads/`。Safari 命令要求 macOS/Xcode，生成工程 ZIP并编译未签名容器。
源代码包可以重现；Xcode 工程生成器本身不保证项目 ID 或编译结果逐字节一致。

Safari 本地未签名开发可能需要开启 Allow Unsigned Extensions，此设置在退出
Safari 后会复位。发布需要遵循 Apple 签名/容器分发流程，不会由网页自动安装。
如果浏览器拒绝 tab-scoped rule 或权限，扩展报错，不降级为全站匹配。

## 不解决的问题

服务器 OPTIONS 必须已返回成功状态。扩展不伪造 OPTIONS 的 401/403/404 为 200，
不修改 Authorization/Origin/cookie、重定向或 TLS 校验。无效 key、额度、网络、
证书和服务商拒绝不属于 CORS 响应头问题。`file://` 不是支持的工作台 origin。

## 验证范围

- 真实 Chromium 扩展环境验证：普通请求因 CORS 失败、启用后预检通过、完整 URL / tab
  隔离、真实 401 保留、OPTIONS 拒绝仍失败、增量 SSE 和关闭后恢复阻止。
- Firefox：web-ext lint 0 错误 / 0 警告；尚不代表 Firefox 实机跨域运行验证。
- Safari：Xcode macOS Release 容器构建成功；尚未验证 Safari 内实际启用后的 CORS
  行为，也未做签名分发、真实 iPhone 验证。WebKit 自动化不能代替 Safari 扩展测试。
- Chrome / Edge 共享构建，但 Chromium 自动化不等于已验证日常 Edge 配置。

官方能力参考：[Chrome DNR](https://developer.chrome.com/docs/extensions/reference/api/declarativeNetRequest)、
[Firefox 临时安装](https://extensionworkshop.com/documentation/develop/temporary-installation-in-firefox/)、
[Safari 扩展打包](https://developer.apple.com/documentation/safariservices/packaging-a-web-extension-for-safari)。
