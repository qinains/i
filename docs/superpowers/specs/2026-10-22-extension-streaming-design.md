# Mac Edge 网络助手扩展与流式请求设计

状态：用户已确认优先采用 Mac Edge 安装专用扩展；本文待用户审阅。本轮尚未编写扩展或流式产品代码。

## 1. 目标与范围

用户要求自行制作浏览器插件，在 Harness 页面下载并引导安装，同时支持真正的 AI 流式请求。经讨论，当前目标确定为 Mac 上的 Edge，兼顾桌面 Chrome。

本设计取代本轮的 iPhone 同源转发优先路线。`2026-10-22-mobile-relay-streaming-design.md` 保留为暂缓方案，不在本轮引入 relay 服务器、relay token 或公网部署。不能声称桌面 ZIP 可直接装到 iPhone Chrome/Edge。

成功标准：用户从页面下载 ZIP，按向导安装，在扩展中授权当前 Harness 页面和选定模型 API，然后无需关闭全局 CORS、无需 debugger 权限，收到实时模型文本，正常执行完整工具批次并保存结果。

## 2. 方案选择

采用「受限扩展网络助手」，不采用修改所有网页 CORS 头或伪造 4xx 状态的通用扩展。扩展自己的页面可在取得 host permission 后请求模型 API；content script 仅负责通信，不能假设它自身免于 CORS。

Manifest V3 service worker 有生命周期限制，不将整个长时模型请求绑在 worker 上。Fetch 实际由扩展自己的「网络助手」标签页执行；service worker 仅负责打开/定位该页及必要的用户操作。该助手页需要保持打开，可以留在后台；关闭时立即失败/取消，不自动重发。

此取舍多一个扩展标签页，但避免为延长后台生命周期请求 debugger 权限、假称打开一个 port 就能永久保活，或为慢首字模型制造隐蔽的 30 秒失败。

## 3. 组件和数据流

```text
Harness Web UI / WASM Agent
  → 当前页面 content-script bridge
  → chrome.runtime.Port（结构化分块消息）
  → 已授权的扩展网络助手页 Fetch
  → HTTPS Chat Completions API
  → 响应字节块原路返回
  → 共享 SSE 解析与 delta 拼装 → UI / 完整 Agent 响应
```

现有 Agent、工作区文件工具、审批、OPFS、图片、ZIP 均保留在网页/WASM，不移入扩展。扩展不提供任意网页抓取、shell、文件访问或远端代码执行。

源码新增独立 `extensions/harness-network/`；网页新增扩展连接适配器与安装向导；SSE 解码与 Chat Completions delta 拼装保持 provider 边界，不在两个传输模式中复制。Rust provider 配置负责产生相同模型请求、接收规范化响应；共享 Native Agent 循环不另写一份。

## 4. 授权与通信边界

- 不申请 `<all_urls>` 常驻访问、debugger、cookies、webRequest 或全局响应修改权限。需要 scripting、storage、activeTab 及按需申请的目标 host permission；storage 只保存授权与非敏感设置。
- 安装后默认未授权，不自动访问当前页面保存的 key，不扫描其他标签页。localhost/127.0.0.1 页面可发现已安装扩展，但「发现」不等于有网络调用权限。
- 用户在扩展页面确认一对授权：完整 Harness origin（含端口）及完整模型 HTTPS endpoint。Chrome 的 host permission 按所需 host 申请；运行时仍精确检查 origin 与 endpoint，不能因浏览器 host 匹配较宽而扩大业务授权。
- 新增非 localhost 的 HTTPS Harness 网站时，须在其标签页由用户点击扩展按钮并授权注入；安装后不向任意 HTTPS 页面预注入脚本。
- 转发仅接受 POST JSON 模型请求；目标为已授权的完整 `/chat/completions` endpoint，不允许任意 URL、任意方法或浏览器提供的任意请求头。拒绝 URL 用户名/密码、query、fragment、非 HTTPS 上游及 loopback/私有地址字面量。开发测试使用单独 fixture，不给正式扩展添加「允许任意内网」开关。
- 只发必要 Content-Type、Accept 和 Authorization，使用 `credentials:omit`，重定向一律拒绝；不转发网页 cookie、Origin 或扩展状态。不修改真实 HTTP 状态和正文错误。
- 每次消息验证协议版本、类型、请求 ID、顺序号及大小。扩展端从浏览器提供的 sender 取得 tab/frame/origin，不信任网页消息自报的来源。仅授权顶层 frame 可调用，不能由 iframe、其他 origin 或其他 tab 取消请求。
- 页面与 content script 使用仅限当前 window/精确 origin 的消息，并完成每文档会话握手；这不是对同源 XSS 的隔离保证。安装检测不执行网页任意指令；假握手不能绕过扩展端的浏览器来源与授权检查。
- 授权列表可在扩展页撤销；撤销后立即中断对应活动请求。扩展不记录模型文本、Authorization、key、请求体或完整传输消息。

## 5. 分块、取消与生命周期

每个网页文档一个连接会话，每个模型请求一个不可复用 ID。协议至少含握手、开始、响应元数据、请求/响应数据块、ACK、完成、错误及取消。头部/正文不能跨不同请求混用。

Port 传输采用 JSON 可序列化数据。字节分片以 Base64 编码，每片原始字节最多 64 KiB，双向流最多 4 个未确认分片；累积上限按解码后的字节检查。不能一次发送整个 48 MiB 文本，避免越过 Chromium 消息大小上限或产生无界缓冲。请求可先按限额收齐后发送；响应必须边读取边转发，并通过 ACK 施加背压。

每个模型请求总期限 180 秒，请求体和响应体分别最多 48 MiB。每个页面最多一个活动任务，扩展全局最多 4 个模型请求。取消、网页刷新/关闭、Port 断开、助手页关闭、权限撤销或超限均使对应请求失败并尽可能 abort 上游。不能把正常 EOF 和异常断开混为一谈，也不能静默改为直连或自动重试。

助手标签页被浏览器丢弃或进程终止时只保证页面收到明确失败/断开，不承诺手机或桌面后台永不被挂起。已取消请求的晚到字节不得写入新请求。

## 6. SSE 与工具调用

新增 `stream` 配置，默认 true，可关闭。现有 direct 与新增 extension 传输共用相同流解析逻辑；使用 Fetch + AbortController，不使用会自动重连的 EventSource。

- UTF-8 增量解码，允许中文字符/JSON/事件被任意网络字节边界拆开。
- 正确处理 LF/CRLF、注释心跳、多行 data、单 chunk 多事件、空行事件边界与 `[DONE]`；有界保留未完成事件。
- 文字 delta 立即追加到同一 assistant 文本块；最终完整响应交给 Agent 后不重复输出一遍。
- tool_calls 按 index 拼接 ID、名称、arguments 原文；只支持 choice index=0，拒绝冲突或不支持的多 choice。保留 reasoning_content 等回放字段，不把它们作为工具指令或自动展示隐藏推理。
- 完整 finish_reason、结束标记与全部工具参数校验通过后，才进入既有完整批次验证及审批。EOF、畸形/错误事件、length 截断或取消时不执行其中任何工具。
- 支持可选 usage 块，不强制给所有服务发送 usage 扩展开关；不编造用量。
- HTTP 非 2xx 错误按真实状态显示，正文限额且脱敏；服务回 JSON 而非请求的 SSE 时明确报错，用户可关闭 stream 后自行重试，不自动产生第二次请求。

取消或流错误可保留已显示文字并标为未完成，但不把该半截 assistant 当完成历史交给模型。已完成的文件副作用不因取消回滚。

## 7. 页面下载与安装向导

提供明确入口「下载 Harness 网络助手」「安装指南」「连接扩展」「重新检测」。状态区区分未安装、已安装未授权、等待助手页、已连接、版本不兼容及已断开，不能只显示笼统已安装。

ZIP 通过可重现构建脚本生成到页面可下载目录，独立于工作区 ZIP。包内包含 manifest、助手页/脚本、content script、图标及中文安装说明；不包含开发依赖、测试数据、用户文件、API key 或机器路径。构建验证 manifest 引用均存在，页面展示扩展版本及 SHA-256。

安装步骤：下载 → 解压到固定目录 → 打开 `edge://extensions`（Chrome 为 `chrome://extensions`）→ 开发者模式 → 加载已解压扩展 → 选择包含 manifest.json 的目录 → 刷新 Harness → 点击扩展授权当前页面和 API → 保持网络助手页打开 → 检测连接。

网页不能静默安装扩展，也不能保证按钮直接导航到浏览器内部页面；向导提供可复制地址与人工步骤。首次不发布到应用商店，不声称签名 ZIP 可一键安装。更新后提示在扩展管理页重新加载，并重新检测协议版本。

不支持此安装方式的平台（尤其 iPhone）显示兼容说明而不是假成功按钮；桌面 Chrome/Edge 的最终兼容性以实际验证为准。

## 8. 配置与兼容

localStorage 配置升级为 version=2，新增 `transport: direct|extension`、`stream:boolean`。旧 version=1 配置保留 API URL、模型、key、语言和限制，迁移 transport 为 direct，stream 使用新的默认 true。不自动更换模式或把旧 key 发给未授权扩展。

新用户默认 direct + stream；需要跨域支持时明确切换 extension 并完成授权。模式切换清理活动传输状态，不自动重放旧任务；切换 API/model 仍新建模型会话。

Remember 仍为显式开关，新增配置一并保存；关闭删除记录，Clear 清理保存和活动配置但不删 OPFS。扩展只为当前请求在内存使用 key，不另行持久化；持久授权仅记录网页 origin、模型 endpoint 和权限选择，不含 key。

不改变 Native CLI/provider 的协议与行为；不扩修已记录的 4 项继承原生网络失败。文档不能把扩展测试或 mock 模型测试称为真实密钥已验证。

## 9. 验证与验收

1. SSE 单元测试覆盖单字节中文切块、CRLF/心跳/多行、交错工具分片、usage、错误事件、缺失 DONE 与结束原因、超限和取消。
2. 在真实 Chromium 扩展环境中加载生成 ZIP 解压后的目录，而不是只 mock chrome API；本地 API 不返回 CORS 许可，普通 direct 应失败、授权 extension 应成功。
3. 增量证明采用服务端闸门：首段已返回、尾段尚未放行时，页面必须已显示首段；非流式模式仍能工作。
4. 验证未授权 origin/endpoint、iframe、伪造来源、错误版本/顺序/ID、真实 401、重定向、请求/响应限额和权限拒绝均失败关闭。
5. 验证 port 中断、撤权、关闭助手页、用户取消、中途断流，不执行未完成工具、不串流到新任务、不自动重试；大消息分块有背压。
6. 生成包结构、版本/哈希、下载按钮和安装指南可复现；配置 v1→v2 的记忆/清除，以及 OPFS/审批/ZIP 现有回归通过。
7. 在 Mac Edge 安装并验证权限与实际流式路径。可以先用独立浏览器 profile 与本地 HTTPS fixture；使用真实付费 API 或读取用户正式 key 需要用户主动操作，不将测试 profile 中的结果冒充用户日常 profile 已安装。

## 10. 参考

- [Chrome 扩展跨域网络请求](https://developer.chrome.com/docs/extensions/develop/concepts/network-requests)
- [扩展消息通信](https://developer.chrome.com/docs/extensions/develop/concepts/messaging)
- [MV3 service worker 生命周期](https://developer.chrome.com/docs/extensions/develop/concepts/service-workers/lifecycle)
- [Edge 本地加载扩展](https://learn.microsoft.com/en-us/microsoft-edge/extensions-chromium/getting-started/extension-sideloading)
- [SSE 格式](https://developer.mozilla.org/en-US/docs/Web/API/Server-sent_events/Using_server-sent_events)
