# iPhone 同源转发与流式回复设计

状态：会话方案已获用户确认，本文待用户审阅；尚未开始本轮产品代码改动。

## 目标与本轮范围

用户需要在 iPhone 的 Chrome/Edge 上使用现有 WASM Harness，调用不允许网页直连的正式模型 API，并实时看到流式回复。用户已确认优先提供 HTTPS 同源转发，桌面扩展作为可选后续增强。

本轮交付：同源模型 HTTP 转发、真正增量的 Chat Completions SSE、浏览器配置与状态集成、手机部署说明和可重现测试。不在本轮制作或发布浏览器扩展，不假设手机能安装桌面扩展 ZIP。

## 当前实现与问题

- `src/browser/provider.rs` 固定发送 `stream:false`。
- `web/platform.js` 虽读取 ReadableStream，但会收齐响应后拼成字符串，不是增量展示。
- `src/browser/runtime.rs` 等待完整 JSON，再将响应交给共享 Agent。
- 页面与模型 API 跨域；已实测阿里云 Coding Plan 端点的无密钥 OPTIONS 返回 401 且没有允许跨域的响应头，因此当前浏览器直连无法完成预检。这不能证明用户 API key 无效。
- 当前本地 `web/tests/mock-server.mjs` 只是测试 fixture，不能直接暴露为生产代理。

## 架构与部署

```text
iPhone / 桌面浏览器
  ├─ HTTPS 页面：WASM Agent、审批、OPFS、ZIP
  └─ 同源 POST /api/chat/completions
       └─ 受限转发服务 → 已配置的 HTTPS 模型端点
```

新增独立 Node.js 服务，生产依赖限定为标准库及明确用于 IP 地址校验的小型依赖；最低 Node.js 24。它服务 Web 静态产物与 `/api`，不运行 Agent、插件、shell 或文件工具。首期通过 Caddy/Nginx 等可信反向代理提供公开 HTTPS；本机调试允许 loopback HTTP。默认监听 `127.0.0.1`，不自动打开公网端口或修改用户代理配置。

手机需访问可达的 HTTPS 域名；iPhone 上的 `127.0.0.1` 指向手机自身。局域网普通 HTTP 地址不能作为 OPFS 已可用的保证。域名、TLS、真实部署凭据尚未提供，因此本轮能交付部署模板和本机验证，不承诺已发布可从手机访问的公网服务。

浏览器有两个明确模式：`direct`（现有直连，需要上游 CORS）和 `relay`（当前页面同源转发）。不在失败后静默切换模式、目标或自动重放请求；转发模式不改变真实 Base URL 的含义。

## 转发接口与访问控制

- 唯一路由：`POST /api/chat/completions`；请求体为既有模型 JSON。禁止由用户指定 HTTP 方法、任意请求路径或任意转发头。
- `Authorization: Bearer ...` 为用户的模型 API key；`X-Harness-Upstream` 为完整 Chat Completions HTTPS 端点；`X-Harness-Relay-Token` 为部署方配置的独立访问令牌。三个值均不进入日志或错误输出。
- 服务启动必须提供独立的高熵 relay token、页面公开 origin，以及完整模型端点 allowlist；缺少必要配置时失败关闭。页面 origin 校验不是身份认证，不能代替令牌。
- 只允许与服务端 allowlist 精确匹配的 HTTPS 端点；URL 不允许用户信息、query、fragment 或非预期端口，不接受以字符串前缀冒充同源。服务端配置不是浏览器提交后自动获批的授权。
- 每次连接前解析 DNS 并验证地址，拒绝 loopback、私网、链路本地、组播、保留地址及其 IPv4-mapped IPv6 形式；实际连接固定到已验证地址，同时保留原 hostname 的 TLS/SNI/证书校验。禁止重定向，避免 DNS 二次解析与重绑定绕过。测试使用注入的 fixture transport，不提供生产 CLI 的「允许私网」后门。
- 上游只接收必要的 Authorization、Content-Type 与 Accept，不转发浏览器 cookie、relay token、Origin、Host 或其他任意头。浏览器请求同源 relay 时也不依赖 cookie 身份。
- 拒绝不匹配的 Origin、错误令牌、非法配置、未知路由和非法 Content-Type；比较 token 时避免简单字符串提前退出。限制请求速率和并发，不能做匿名通用代理。
- 默认全服务最多 4 个进行中的上游请求，每分钟最多接受 60 个模型请求；超限返回 429。每请求期限 180 秒，总请求体与总响应体各最多 48 MiB。客户端断开、取消或超限立即终止上游，不自动重试。
- 静态文件只暴露明确的页面资源和 WASM 产物，不能泄漏服务脚本、部署配置、隐藏文件或任意父目录文件。

## 真正的流式路径

浏览器默认开启 `stream:true`，提供可关闭选项。直连与 relay 共用同一 SSE 解析/拼装逻辑；relay 原样增量转发，不先收齐、不拼接成单个 JSON、不做状态码伪装。

使用 Fetch 流而非会自动重连的 EventSource：保留 POST、鉴权和 AbortController。UTF-8 解码跨 chunk 保留状态；SSE 支持 LF/CRLF、注释/心跳、多行 data、事件被任意字节边界切开。HTTP 字节 chunk 不等于完整事件。

共享浏览器 provider 负责将 Chat Completions delta 归并成当前 Agent 接受的完整 normalized response：

1. 文字 delta 立即发送带本次请求 ID 的 UI 事件，追加到同一个 assistant 文本块，不为每个 token 新增一行。
2. 完整响应交回 Agent 时不重复显示已展示文字。普通非流式响应仍沿用完整展示。
3. tool_calls 按 index 归并，保留调用 ID、名称与 arguments 原文；分片参数未完成不能执行。全部调用、结束原因和流完成标记均通过验证后，才进入原有完整批次校验和审批。
4. 单 choice（index=0）为本阶段支持范围，出现不支持的多 choice 或结构冲突明确报错。保留 provider 输出的 reasoning_content 等可回放字段，不把它们当工具指令，不擅自向 UI 展示隐藏推理。
5. 支持结束块及可选 usage 块；不强制发送可能不兼容的 usage 请求选项。严格要求 `[DONE]` 与完整 finish_reason；EOF、截断、畸形事件、错误事件或取消不算成功，不执行其中的工具。
6. 累积流数据仍受 48 MiB 限制。错误保持真实 HTTP 状态；非 2xx 的错误正文经限额和密钥脱敏后显示，不将它误当 SSE。

取消由页面 AbortController 贯穿读取、relay 和上游。晚到事件以请求 ID 与活动状态过滤。已显示文字可保留并标为「未完成/已取消」，但不作为完整 assistant 写入模型历史；不能混入下次请求，也不能因为取消而回滚已完成的文件操作。

## 配置、手机 UI 与凭据

新增配置项：`transport: direct|relay`、`stream: boolean`、`relay_token: string`。新配置默认 relay + stream，界面明确标注服务未配置或 token 缺失，不能假装可用。

已有 localStorage version=1 配置迁移时保留 API URL、模型、key 与其余设置，transport 维持 direct，stream 使用新默认 true；不给旧 key 静默添加新的网络接收方。新增字段随整套配置保存为 version=2，仍仅在用户勾选 Remember 后持久化。直接模式不向上游发送 relay token；relay token 仅发送到当前页面 origin，不提供任意第三方 relay URL 输入。

API key 与 relay token 默认仅在页面内存中；Remember 包含两者并明确提示。取消 Remember 删除保存记录；Clear 清除浏览器保存与活动配置，不删除 OPFS。任何凭据都不进入 ZIP、模型 body、OPFS、会话或应用日志；服务端只为当前请求在内存使用，不持久保存模型 key。服务端部署配置中的 relay token 属于运维秘密，不提供给模型。

终端展示连接、等待首字、生成中、工具调用、完成/取消/错误等真实状态。手机纵向屏幕上设置区可折叠，输入框/取消/审批/下载可操作，不被键盘遮住的体验需实机确认。保留安全的 textContent 渲染，模型输出不是 HTML。

## 错误与证据边界

已有共享核心文件工具、审批、OPFS、ZIP、Native CLI/provider 行为保持不变。本轮不修复已记录的 4 项继承原生网络失败，也不据浏览器测试声称全仓库测试已全绿。

浏览器报 Fetch failed 时区分直连 CORS、relay 不存在/鉴权失败、上游非 2xx 和中途流断开；只有能从运行时取得的状态才确定报告，不能凭 TypeError 猜测 CORS。日志不打印请求头、模型文本、上传文件、配置值或含令牌的 URL。

## 验证与验收

- 共享 SSE 测试：中文 UTF-8 单字节切块、多行/CRLF/注释、多个事件同 chunk、多个交错 tool_calls、畸形/缺失结束标记、usage 与错误事件。
- relay 集成测试：真实 HTTP 请求验证令牌/Origin/allowlist、受限头、SSRF 地址分类及 DNS 固定、禁止重定向、静态目录隔离、限流、大小/时间/并发上限和断开上游。
- 增量证据：本地 fixture 发首段后等待测试许可才发送尾段；必须在尾段未发送时看到页面首段，不能只靠最终文本判断「流式成功」。
- 完整工具批次测试：分片参数完整时可审批并写入；EOF/错误结束/取消时文件未改，之后也不执行晚到工具。
- 浏览器回归：直连与 relay，流式与非流式，配置 v1→v2 迁移/记忆/清除，OPFS 持久化与 ZIP 下载均验证。
- Chromium 与 WebKit/iPhone viewport 自动化通过后，只能称模拟与引擎测试通过；用户在实际 iPhone Chrome/Edge 上连接 HTTPS 部署完成验收前，不能声称实机支持已验证。缺少可访问域名/证书或用户手机时记录为明确交付边界，不读取用户真实 key 代跑付费请求。

## 参考

- [Chrome 跨域扩展请求](https://developer.chrome.com/docs/extensions/develop/concepts/network-requests)：桌面扩展可作为后续传输适配，但不是本轮手机依赖。
- [MDN SSE](https://developer.mozilla.org/en-US/docs/Web/API/Server-sent_events/Using_server-sent_events)：UTF-8、多行 data、事件边界及心跳约定。
- [Node.js HTTPS](https://nodejs.org/api/https.html)：上游 HTTPS 请求与流式转发；实施时核对所选最低版本接口。
