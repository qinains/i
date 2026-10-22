# PWA Direct Streaming Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 交付默认直连、实时流式输出、可离线启动的 Harness PWA，另提供显式启用的自有同源转发服务。

**Architecture:** 保留 WASM Agent 和工作区，将 Fetch/SSE 处理放在浏览器传输边界，向 Rust 返回完整 Chat Completions JSON 并向 UI 发送增量事件。PWA 只缓存版本化静态资源，可选 Node 服务只做受限模型转发与静态发布。

**Tech Stack:** Rust/wasm-bindgen、浏览器 ES modules/Fetch/Service Worker、Node.js 24、node:test、Playwright。

**Spec:** `docs/superpowers/specs/2026-10-22-pwa-direct-streaming-design.md`

## Global Constraints

- 默认 direct + stream；relay 由用户显式选择，绝不自动切换、自动重试或改换密钥接收方。
- 请求、响应各最多 48 MiB，总期限 180 秒；Native CLI 行为不变，shell 继续禁用。
- 完整 finish_reason、[DONE]、参数和工具批次校验之前不得执行工具；半截 assistant 不进入完整模型历史。
- 配置 version=2；Remember 显式选择；Clear 不删 OPFS；日志、缓存、ZIP 不含密钥。
- HTTPS 生产发布，支持子路径；活动任务不自动升级/刷新；不承诺永久存储或后台持续生成。
- 不改用户已有 `examples/plugins.json` 等无关变更；仅按明确路径提交本轮文件。

## Review Focus

- 旧版本已安装 worker 遇到部署缺文件：新安装失败应保留旧应用，不产生混合 JS/WASM（Task 5）。
- 多标签页、审批中收到更新：不得刷新活动任务或让另一个标签强制换代（Task 5）。
- 首段文本后取消、晚到尾段：不能重复输出、串入下一次任务或执行半截工具（Task 3）。
- 攻击者通过 DNS/IPv4-mapped IPv6/重定向访问本机：转发必须失败关闭并固定已校验地址（Task 4）。
- 安装态没有普通标签页配置或 OPFS：显示实际空状态，不偷偷迁移 key 或声称文件丢失（Task 6）。

## Task 1: 版本化配置与显式传输选项

**Files:** Modify `web/config.js`, `web/app.js`, `web/index.html`, `src/browser/provider.rs`; test `web/tests/config.test.mjs`。

**Interfaces:** `BrowserConfig` 新增 `transport: String`, `stream: bool`, `relay_token: String`；JS 默认同名字段。`loadConfiguration(storage)` 接受 v1/v2，`saveConfiguration(storage, settings)` 写 v2。relay endpoint 固定为页面发布根下 `api/chat/completions`，不提供第三方 relay URL。

- [ ] 添加配置测试：v1 保留全部旧值并补 direct/true/空 token；v2 往返；非法模式/字段类型失败；取消记忆和清理只删除本应用记录。
- [ ] 运行 `cd web && npm test`，确认新断言 RED。
- [ ] 实现配置校验、表单、模式说明；Rust 保留现有 endpoint 校验，HTTPS 为正式上游要求，仅 loopback HTTP 允许开发；活动任务不可改配置，切换模式/API/model 新建会话；不自动发请求。
- [ ] 运行同一命令确认 GREEN，并执行 WASM cargo check。
- [ ] 仅提交本任务文件：`feat: add explicit browser transport and streaming settings`。

## Task 2: 有界 SSE 解码与响应归并

**Files:** Create `web/sse.js`, `web/tests/sse.test.mjs`。

**Interfaces:** `readChatCompletion(reader, {signal,onDelta,maxBytes}) -> Promise<object>` 消费 Uint8Array 流，返回完整 Chat Completions 对象；`onDelta(text: string)` 只收到可展示文本。非流式不调用本接口。

- [ ] 编写测试：单字节中文、CRLF/多行 data/注释、同块多事件、交错工具参数、可选 usage、reasoning_content 回放字段、未知 choice/冲突、length/缺 DONE/缺 finish_reason/错误事件、48 MiB 上限、abort；全部失败路径不得产生成功结果。
- [ ] 运行 `cd web && node --test tests/sse.test.mjs`，确认缺实现 RED。
- [ ] 实现增量 UTF-8/SSE 解析和单 choice=0 累积，按 index 归并工具，保留兼容回放字段；限制未完成事件与总响应大小，不展示 reasoning，不提前 dispatch 工具。
- [ ] 运行上述单测及 `npm test` 确认 GREEN。
- [ ] 提交：`feat: decode bounded streaming chat completions`。

## Task 3: Fetch/WASM/UI 真正增量链路

**Files:** Modify `web/platform.js`, `src/browser/runtime.rs`, `src/platform/wasm.rs`, `web/app.js`, `web/style.css`, `web/tests/mock-server.mjs`, `web/tests/agent.spec.mjs`, `web/tests/platform.test.mjs`。

**Interfaces:** `fetchModel(url, body, key, controller, options, onDelta) -> Promise<string>` 返回完整 JSON 字符串；options 为 `{transport,relayUrl,relayToken}`。Rust bridge 将 delta 转成 `{type:'assistant_delta',request_id,text}`，另发对应 stream start/end/error 事件；最终 Terminal 输出仍保留语义，但不得再次渲染已经显示的相同完整文本。非流式路径不变。

- [ ] 添加 RED 测试：fixture 在首段后等待闸门，尾段未释放时页面已显示首段；普通 JSON 不自动重试；非 2xx 保留真实状态且 key 脱敏；取消/中断后无工具副作用，下一请求不混流；JSON 非流式仍正常。
- [ ] 运行 Node 新测试与对应 Playwright 用例，确认预期失败。
- [ ] 接入 Task 2，增加 180 秒 deadline、48 MiB 限额、Accept、请求隔离及 reader abort；HTTP 错误先有界读取再脱敏。relay token 只给同源 relay，模型 key 不进入 body。Rust 只有完整解析成功后才交回共享 Agent。
- [ ] 实现单块增量渲染、未完成标记、取消清理与无重复最终输出；TypeError 显示可能的网络/CORS问题，不断言根因、不重放。
- [ ] 构建 `sh scripts/build_web.sh`；运行 `cd web && npm test && npm run test:browser`，确认新用例与现有 OPFS/审批/ZIP 回归 GREEN。
- [ ] 提交：`feat: stream browser model output without partial tool execution`。

## Task 4: 独立可选同源转发服务

**Files:** Create `server/package.json`, `server/package-lock.json`, `server/addresses.mjs`, `server/relay.mjs`, `server/main.mjs`, `server/tests/relay.test.mjs`, `server/tests/addresses.test.mjs`。

**Interfaces:** `createRelayServer({token,origin,endpoints,staticRoot,lookup,connect}) -> http.Server`；lookup/connect 仅供程序依赖注入测试，不提供放开私网的部署开关。生产使用固定 DNS 校验的 HTTPS 连接。启动配置为 `HARNESS_RELAY_TOKEN`, `HARNESS_PUBLIC_ORIGIN`, `HARNESS_UPSTREAMS`（完整端点 JSON 数组）, `HARNESS_STATIC_ROOT`；默认 `127.0.0.1:8080`。

- [ ] 先写 RED 测试：令牌/Origin/allowlist、受限头、SSRF IPv4/IPv6/mapped/混合 DNS、DNS 固定/TLS hostname、重定向拒绝、静态穿越/隐藏文件/配置泄漏、并发 4、每分钟 60、请求响应各 48 MiB、180 秒期限、断开取消；测试用小限额/假时钟加速，生产默认不可降低保护。
- [ ] 执行 `cd server && npm test` 确认 RED。
- [ ] Node >=24 标准库实现；仅地址分类引入锁定版本的小型成熟依赖。令牌高熵检查与 timing-safe 比较，缺配置拒绝启动；JSON POST 唯一模型路由 `api/chat/completions`，允许配置的发布根路径；不做任意抓取。
- [ ] 实现受背压约束的逐块转发、真实状态、限额、限流、abort、SSRF 地址分类和固定解析结果；静态仅发布构建白名单，不扫整个仓库；服务端不记请求/响应内容或凭据。
- [ ] 运行服务单测及浏览器同源 relay fixture：direct 非 CORS 失败、显式 relay 成功且首段不缓冲；服务不运行时静态 direct 仍可用。
- [ ] 提交：`feat: provide optional authenticated same-origin model relay`。

## Task 5: 可离线启动的版本化 PWA

**Files:** Create `web/manifest.webmanifest`, `web/pwa.js`, `web/sw.js`, `web/icons/icon.svg`, `web/tests/pwa.test.mjs`, `web/tests/pwa.spec.mjs`, `scripts/build_pwa.mjs`; modify `scripts/build_web.sh`, `web/index.html`, `web/app.js`, `web/style.css`, `web/tests/mock-server.mjs`。

**Interfaces:** `registerPwa({isBusy,onStatus}) -> Promise<void>` 管理安装/更新；`scripts/build_pwa.mjs` 生成发布资源白名单与内容哈希版本，包含 wasm-bindgen 递归 snippet 引用。图标由代码原生 SVG 构建并生成 192/512 PNG（选用可重现本地渲染工具，不调用图像生成服务）。

- [ ] 写 RED 测试：manifest scope/start_url、全部静态引用存在、子路径加载、缓存中无 key/POST/模型响应、在线首次缓存后离线重开文件/ZIP可用、缺资源安装失败保留旧版本、无旧 HTML 假冒脚本/API。
- [ ] 添加更新测试：活动生成/审批与多个标签页时不强制刷新；空闲经确认才激活；只清理本应用前缀缓存。运行 Node/Playwright 确认 RED。
- [ ] 构建白名单/version，预缓存全部成功才安装；Service Worker 只接管范围内白名单同源 GET，不缓存任何模型流或配置；根路径/子路径均使用相对发布根。
- [ ] 实现安装按钮能力检测、iPhone 手动指南、离线状态和用户确认更新；跨标签页活动协调采用心跳/失效租约，未知活动状态延后更新，不阻塞已有工作。
- [ ] 构建后执行 Node 与 Playwright PWA 测试，验证更新前后 JS/WASM 版本一致；现有全套浏览器回归通过。
- [ ] 提交：`feat: add offline PWA installation and safe app updates`。

## Task 6: 手机体验、部署文档与最终验证

**Files:** Modify `web/style.css`, `web/app.js`, `docs/wasm.md`, `README.md`, `README.zh-CN.md`, `web/playwright.config.mjs`; create `web/tests/mobile.spec.mjs`, `server/README.md`。

**Interfaces:** 维持前述公共接口；WebKit 测试独立 project，既有 Chromium project 不变；未验证实际 iPhone 时文档明确说明。

- [ ] 添加 RED 测试：手机尺寸下设置、键盘输入、审批、取消、下载可达；独立新 profile 显示未配置/空工作区且不自动读取普通 profile 数据。
- [ ] 修复手机布局，补充 HTTPS 静态部署、子路径、可选服务令牌/allowlist、反向代理关闭 SSE 缓冲、安装与更新、存储/锁屏限制、ZIP 备份、真实 CORS 排查说明。
- [ ] 执行 `cd web && npm test`、`sh scripts/build_web.sh`、Chromium/WebKit 浏览器套件、`cd server && npm test`、`cargo check --target wasm32-unknown-unknown --no-default-features --features wasm --lib` 及共享核心/Native 定向回归。端口已被旧 fixture 占用时先确认来源，测试不得误用旧进程伪造成功。
- [ ] 记录精确通过数、失败与环境缺口；已知四项 Native network 继承失败单列，不称全仓库全绿。不读取用户正式 API key，不代跑付费请求，不自动公开服务。
- [ ] 按 executing-plans 要求做一次独立最终代码审查，修复阻断项后重新验证；仅提交本轮文件，不擅自合并或推送。

## 审阅与执行

自审覆盖设计全部章节；三个可独立验收里程碑为直连流式（1–3）、可选服务（4）、PWA 与手机交付（5–6）。既定执行方式为当前会话由主代理实现，最后一次独立审查。请用户审阅本计划并确认范围；确认前不开始产品实现。
