# CORS Helper：安装、构建与边界

它是可选的浏览器扩展，不是 agent 的 `.hplugin.zip` 工具。默认关闭，采用
Manifest V3 declarativeNetRequest 修改响应头，按标签页、API 完整 URL 和请求页面
origin 限制。普通模式不读取 API key、正文与模型回复；Chromium 的高级预检修复
需要更强的调试器权限，具体元数据访问边界见下文。

## 下载与安装

工作台：设置 → 模型连接 → 桌面浏览器 CORS 扩展。

- Chrome / Edge：下载 chromium ZIP，解压；扩展管理页开启开发者模式，加载已解压扩展。
- Firefox 142+：下载 firefox ZIP，在 `about:debugging` → 此 Firefox → 临时载入附加组件。
  未签名开发包只在当前浏览器运行期间有效；永久分发需要 Mozilla 签名。
- Safari：源码 ZIP 不能直接安装，使用 Xcode 转为容器应用后在 Safari 启用。
  仓库脚本也可以生成完整 macOS Xcode 工程 ZIP。iPhone 需要签名容器应用。

先在工作台保存 API 设置，再点击扩展图标：默认仅读取页面
公开的、生效的完整请求 URL，点击“启动”并授权即可，不必复制地址。未识别到时
才展开手动填写，必须填完整请求路径而非 API 根地址；不要填写密钥。授权后手动重试，网页保持
Direct。使用后关闭；浏览器重启后默认关闭，不自动重发旧模型请求。

自动识别使用 `scripting` + `activeTab` 临时权限，只读取页面根节点的
`data-model-api-url` 与 `data-model-transport`，不读取 API key 输入框或浏览器配置存储；
页面只公开已应用的 URL，不公开尚未保存的编辑。中继模式无需启用 CORS 扩展。
这是实现中的读取范围，不代表高权限浏览器扩展天生不能读网页密钥。旧扩展需要
用新版文件覆盖后在扩展管理页重新加载，或移除旧版后加载新版；旧 PWA 也需应用更新。

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

普通模式要求 OPTIONS 成功，仅改响应头；Firefox/Safari 仍遵循此限制。
0.3.1 界面只有一个“启动 / 停止”按钮。Chrome / Edge 点击启动并确认后，一并开启
响应头规则和预检修复；任一步失败会回滚，不留下半启用状态。只为当前标签页和所选 API
合成 OPTIONS 204，不改变真实 POST 的状态、鉴权和内容。启动后回网页手动重试，
无需再次启用。地址变更需先停止再启动；Firefox/Safari 只有普通模式。

不要直接加载仓库 `extensions/cors-helper` 源码目录，否则会缺少生成的 PNG 图标及
浏览器专用配置。先运行 `python3 scripts/build_cors_extension.py`，Chrome 加载
`dist/cors-helper/chromium`，或解压官网 ZIP 后加载其中的目录。

Chrome / Edge 118+ 版本需要安装时声明的高权限 `debugger`，无法将其作为 optional 权限。
调试器可能收到匹配请求的认证头等元数据；实现只筛选并合成预检，不记录或持久化密钥、
正文和回复，不读取/缓存流式正文。关闭浏览器调试横幅、关闭修复/扩展、跨 origin 导航、
关闭标签页会停止修复；不能自动恢复授权。开发者工具或其他调试器可能使开启失败。
此实现边界不代表 debugger 权限天生无法读取页面其他信息。

无效 key、实际 POST 的 401/403、额度、证书、TLS、网络与服务商限制仍原样保留。
不修改实际请求的 Authorization/Origin/cookie。`file://` 不受支持。未使用真实 API key
测试当前厂商接口，只验证了可复现的本地 401 预检及真实请求链路。

## 验证范围

- 真实 Chromium 扩展环境验证：普通请求因 CORS 失败、启用后预检通过、完整 URL / tab
  隔离、真实 401 保留、普通模式 OPTIONS 拒绝仍失败、高级模式合成预检 204、
  用户取消确认、增量 SSE、关闭调试与关闭扩展后恢复阻止；同时验证
  弹窗自动识别、固定宽度、手动回退、中继模式禁用。单元测试覆盖启用途中切换
  页面时撤销过期授权，浏览器测试覆盖仅公开已保存的 URL。
- Firefox：web-ext lint 0 错误 / 0 警告；尚不代表 Firefox 实机跨域运行验证。
- Safari：Xcode macOS Release 容器构建成功；尚未验证 Safari 内实际启用后的 CORS
  行为，也未做签名分发、真实 iPhone 验证。WebKit 自动化不能代替 Safari 扩展测试。
- Chrome / Edge 共享构建，但 Chromium 自动化不等于已验证日常 Edge 配置。

官方能力参考：[Chrome DNR](https://developer.chrome.com/docs/extensions/reference/api/declarativeNetRequest)、
[Firefox 临时安装](https://extensionworkshop.com/documentation/develop/temporary-installation-in-firefox/)、
[Safari 扩展打包](https://developer.apple.com/documentation/safariservices/packaging-a-web-extension-for-safari)。
