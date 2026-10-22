# PWA 优先直连与可选转发设计

状态：用户已确认讨论中的方向；本文待用户审阅，尚未进入实现计划或产品代码阶段。

## 1. 目标与方案选择

用户希望在 iPhone 和电脑使用现有浏览器 WASM Harness，并指出部分模型接口允许跨域。因此采用 PWA 默认直连、真正流式输出、按需启用自有同源转发的方案，不要求所有用户部署服务或安装扩展。

本文替代本轮的 `2026-10-22-extension-streaming-design.md`；不制作扩展 ZIP。之前的 `2026-10-22-mobile-relay-streaming-design.md` 仅作为可选转发的安全约束参考，其「新用户默认 relay」等优先级与本文冲突时以本文为准。

比较：纯 PWA 直连部署最简单，但不能访问不允许 CORS 的模型接口；强制转发增加所有用户的部署成本与密钥接收方；默认直连加显式可选转发兼顾两类接口，故选择第三种。

成功标准：允许 CORS 的接口通过静态 HTTPS 托管的 PWA 即可使用；无需转发配置即可流式展示、审批文件操作并导出 ZIP。不允许 CORS 的接口可在部署自有同源服务后由用户主动切换，不静默降级或重试。

## 2. 数据流与边界

```text
PWA / 普通浏览器页面
  ├─ WASM Agent、审批、OPFS、ZIP、配置
  ├─ direct（默认）→ 允许 CORS 的 HTTPS Chat Completions API
  └─ relay（可选）→ 同源 /api/chat/completions → 已授权 HTTPS 模型 API
```

PWA 是安装与缓存层，不授予跨域特权。WASM 与 Service Worker 不绕过 CORS；不使用 no-cors 伪装成功。不提供任意第三方转发 URL，不在直连失败后自动发送 API key 到其他接收方。

保留现有共享 Agent、OPFS、审批、图片、ZIP 和 shell 禁用策略，不改 Native provider 行为。请求仍为 Chat Completions，不借本轮增加其他模型协议。

## 3. PWA 安装、缓存与更新

新增 Web App Manifest、应用图标、独立显示模式及 Service Worker。支持子路径部署：manifest scope/start_url、缓存资源与 worker 注册均相对 Web 发布根，不假设部署于域名根路径。

预缓存仅包含明确列出的本应用静态壳、JS/CSS、图标与 WASM 构建产物。离线可启动已成功缓存的应用，访问本地文件、上传和导出 ZIP；AI 请求需要联网。首次打开尚未缓存时不承诺离线可用。

模型请求、POST、API 错误、配置、Authorization 与外部资源永不进入 Service Worker Cache Storage。worker 仅处理同源发布范围内的静态白名单 GET；其他请求透传，不以旧 HTML 代替 API 或缺失脚本。

缓存采用构建版本，完整预缓存成功后才可安装。更新可用时提示用户；活动任务期间不自动 reload、skipWaiting 或切换 WASM/JS 版本。用户确认且任务空闲后激活并刷新；仅清理本应用前缀的旧缓存，不删除其他应用缓存、localStorage 或 OPFS。

iPhone 显示 Safari「分享 → 添加到主屏幕」指南；不假设存在通用安装弹窗。支持 beforeinstallprompt 的浏览器可提供安装按钮，其余显示操作指南。HTTPS 为生产要求，localhost 仅用于本机开发；手机不能通过其自身 localhost 访问电脑服务。

安装态与普通标签页是否共享存储不作保证，不能自动复制未知来源的密钥；首次独立打开检查实际配置与文件状态。卸载、系统回收、清站点数据均可能导致数据丢失，继续提供 ZIP 备份，不承诺永久存储或锁屏后台持续生成。

## 4. 流式与工具安全

新增 stream 开关，默认 true，可关闭。direct 与 relay 共用同一增量 SSE 解码器，以 Fetch + AbortController 实现 POST 鉴权、取消，不用自动重连的 EventSource。

- 增量 UTF-8 解码，支持任意字节切片、LF/CRLF、注释心跳、多行 data、一个网络块包含多个事件，残留缓冲有界。
- 文本 delta 立即更新同一个 assistant 输出块；完整响应交给 Agent 时不重复显示。网络结束前须能观察到已渲染的首段。
- 仅支持 choice index=0，tool_calls 按 index 拼接 ID、名称与 arguments，检测冲突；保留必要 provider 回放字段，不自动展示隐藏推理。
- 完整 finish_reason、[DONE]、全部工具参数及既有批次校验通过后才允许审批和执行。截断、length、缺少结束标记、错误、取消均不得执行半截工具。
- usage 可选，不强加可能不兼容的扩展请求参数。请求 stream 却返回普通 JSON 时明确提示关闭 stream 后手动重试，不自动重复付费请求。
- 请求与响应各限 48 MiB，总期限 180 秒；超限、中断、取消均 abort。HTTP 非 2xx 保留真实状态，错误正文限额并脱敏。
- 请求 ID 隔离晚到事件；半截文本标记未完成，不当作完整 assistant 写入历史。既有已完成文件副作用不因取消回滚。

## 5. 配置、凭据与可选服务

localStorage 升级 version=2，新增 transport: direct|relay、stream:boolean、relay_token:string。新用户默认 direct + stream；旧 version=1 保留原字段并迁移为 direct + stream，relay_token 为空，不自动发请求。

Remember 仍显式选择，保存整套配置，包括新增字段；默认仅在内存。取消 Remember 删除保存记录，Clear 同时清理活动配置但不删 OPFS。提示保存的 key 可被同源脚本读取；不把配置或 key 写入静态缓存、模型请求体、ZIP、日志或错误诊断。

直连不发送 relay token；relay token 只发当前页面同源 API。切换模式、API、模型时禁止活动任务修改配置，空闲时开始新模型会话，不重放任务。

可选服务独立部署，静态 PWA 不依赖它启动。沿用 mobile-relay 设计的完整端点 allowlist、独立高熵访问令牌、精确 Origin、HTTPS、DNS 地址校验与连接固定、拒绝私网与重定向、受限头、并发/速率/大小/时间限制、断开取消与静态目录隔离。默认 loopback 监听，不暴露匿名开放代理。服务端与反向代理须逐块转发 SSE、不缓存或压缩缓冲；转发模式界面明确说明模型 key 会经过自有服务。

## 6. 错误与 UI

配置区说明「直连需要接口允许跨域；PWA 不解除限制」。区分可观察的 HTTP 状态、超时、取消、断流和浏览器网络失败；Fetch TypeError 不能单独证明 CORS，提示检查开发者工具、网络与服务端 CORS。错误不触发自动切换、自动重试或向第三方探测接口发送 key。

手机布局保证输入、审批、取消、文件和下载入口可用；保留 textContent 安全渲染。安装、离线与更新状态如实显示，离线发起模型请求给出清晰提示，不影响本地文件工具。

## 7. 验证与交付边界

1. SSE 单测覆盖单字节中文、多行/CRLF/心跳、交错工具分片、usage、错误、截断、大小上限和取消。
2. 真实浏览器直连允许 CORS 的 fixture，并用尾段闸门证明首段先显示；完整工具正常审批，半截工具无副作用。关闭 stream 后非流式回归通过。
3. 非 CORS fixture 直连失败且不自动转发；可选 relay 下成功。覆盖服务鉴权、SSRF/DNS 固定、真实上游错误、取消与不缓冲。
4. manifest、图标与安装资源有效；根路径和子路径均能加载。首次在线后断网重开，WASM 与文件操作可用；未缓存资源不伪装成功。
5. 验证缓存不含任何模型请求或凭据；升级中不混用资源，活动任务不刷新，只清理本应用旧缓存；多标签页更新行为明确可复现。
6. 配置迁移、Remember、Clear、OPFS 持久化、审批、二进制/中文文件与 ZIP 回归通过；Native 已记录失败不冒称已修复。
7. Chromium 与 WebKit 自动化不等于实际 iPhone 验收。缺少手机、可访问 HTTPS 域名或证书时明确列出待验项；不读取用户正式 key 或代跑付费请求，不声称已公网部署。

## 8. 实现前审阅

本文获用户确认后再编写实施计划并选择执行方式。扩展方案暂缓；目前未实现 PWA、SSE 或生产转发服务。
