# 浏览器 WASM

## 显式对话反思

内置声明式分析插件提供 `/reflect [all|user|assistant]`，默认 `all`。每次都需要手动执行并确认，向当前连接的模型额外发送一次请求；不会自动在后台执行，也不会作为排队消息运行。

宿主按 `plugins/reflection-spec.json` 的范围和预算借用投影历史：默认最近最多 40 条、48,000 字节，保留原始消息索引和截断统计；`all` 包含 user、assistant 和 tool。system 主提示、隐藏推理、签名、加密状态、图片和其他二进制内容不交给分析插件。模型请求不带工具，工具调用响应不会执行。

结果仅保存为独立的反思报告，可随会话重新打开；不会加入模型历史、改变系统提示或会话标题，也不会修改工作区文件。停止按钮会取消请求，取消后的迟到结果不会保存。已有连接绑定仍适用，不能借反思把旧对话静默发送到另一个连接。

反思借用当前会话快照，不在等待模型期间占用工作区文件锁。`user` 视图不把已有压缩摘要、参考资料注入或工具图片当作真实用户输入；完整视图保留这些上下文并标注来源，不自动读取旧备份。

构建时 `scripts/build_pwa.mjs` 将规范复制为 `web/reflection.json` 并纳入离线缓存；离线仍不能调用模型。此入口不授予第三方 WASM 工具插件访问聊天、密钥或浏览器存储的权限。若发现反思建议需要修改提示或工作流，先审阅，再手动应用。

## 构建与启动

需要 Rust 1.91+、`wasm32-unknown-unknown`、wasm-pack 和 Node.js 22+；静态站构建、翻译和发布检查使用 Rust 仓库工具，不需要 Python。纯直连的 Web 运行时不需要 npm 或服务端 Agent。

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-pack --locked
./scripts/build_web.sh
node web/serve.mjs
```

默认对主模块和插件校验模块使用 `min-size` profile，不裁剪功能。当前实测体积及 gzip/Brotli 数据、对比命令见 [WASM 体积](wasm-size.md)；`node scripts/measure_wasm.mjs` 可复测。

打开 `http://127.0.0.1:8080`。必须通过 localhost 或 HTTPS 加载，不支持双击 `file://`。构建输出为 `web/pkg/`、`web/assets.json`、`web/service-worker.js` 和 PNG 图标（均不提交生成文件）；脚本禁用额外 wasm-opt 步骤，避免隐式下载优化器，Rust release 优化仍启用。静态发布应只部署 assets.json 中的 assets 加 assets.json，不暴露 tests/node_modules 或仓库文件；支持根路径或 `/harness/` 等子路径，必须原子发布整个构建，不能混用不同版本的 JS/WASM。

原生入口仍为 `cargo run --bin i -- ...`。普通 `cargo build` 只构建根 package；需要 worker 时先 `cargo build -p i-plugins`，或 `cargo build --workspace`。WASM 通过 library 的 `cdylib` 导出，不再使用占位的 i-wasm binary。

## 模型配置与任务

在设置的模型连接中选择接口协议，填写 **API URL**、模型名称和 API key。API URL 可填写根地址或完整请求端点，页面显示实际请求地址，路径只补一次：

| 接口协议 | API URL 示例 | 实际请求地址 |
| --- | --- | --- |
| OpenAI-compatible (Chat Completions) | `https://api.deepseek.com` 或 `https://api.deepseek.com/chat/completions` | `https://api.deepseek.com/chat/completions` |
| OpenAI (Responses) | `https://api.openai.com`、`https://api.openai.com/v1` 或 `https://api.openai.com/v1/responses` | `https://api.openai.com/v1/responses` |
| Anthropic (Messages) | `https://api.anthropic.com`、`https://api.anthropic.com/v1` 或 `https://api.anthropic.com/v1/messages` | `https://api.anthropic.com/v1/messages` |

也支持自定义服务的路径前缀，不根据域名或模型名称猜测协议。Anthropic 使用原生 Messages 请求、system/content blocks、tool_use/tool_result 和图片格式，支持 JSON 和流式响应；续聊保留 thinking/signature 原始块。切换协议或账号仍需新建会话或明确换绑，不自动发送旧历史。浏览器不能直接启动原生进程，但可显式配对同机运行的原生连接插件；原生已有协议不变。

### 可选本地连接插件

设置的 **Local connection plugins** 管理入口与工作区 WASM 工具插件分开。用户需自行安装可信的 `i-chatgpt-plugin` 或 `i-claude-plugin`，运行 `serve --origin <页面确切 origin> --port 0`，打开其 `#connection-plugin=...` 配对链接，再在模型连接中检查插件、登录服务商、选择实际返回的账户/模型并保存更改。配对先创建未保存草稿，不替换已有连接，页面也不自动安装可执行程序。完整命令及 CLI 用法见 [原生连接插件](chatgpt-login.md)。

OAuth 凭据保留在原生插件侧；浏览器使用的是独立的本地配对能力密钥，可按记住配置选项保存。删除连接只清浏览器配对，不等于服务商登出；同一助手实例的账户选择会影响共享它的所有连接。原生插件有当前用户的 OS 权限，**不受 WASM 沙箱隔离**，不能因不转发主 API key 就将其视为无害。

本地助手必须持续运行，手机不能独立使用此模式。HTTPS 访问 loopback 可能需要浏览器本地网络授权，无法绕过安全策略；受阻时使用 CLI 或本机 HTTP 服务的 Web UI 并重新配对对应 origin。ChatGPT 套餐授权路径固定流式且忽略最大输出 token 设置；Claude Console OAuth 使用 API 权限与计费，不是 Pro/Max 额度。真实 OAuth 和模型调用需用户授权验证，离线测试不代表已实测登录。

Responses 是独立协议，并非只替换 URL。使用原生 `input`、`input_text/input_image`、扁平 function schemas、`function_call/function_call_output` 和 `max_output_tokens`，支持普通 JSON 与 typed SSE。函数 schema 显式 `strict: false`，保留现有工具可选参数。工具仍走本地文件系统与审批，不启用服务商托管工具。默认 `store: false`，不使用 `previous_response_id`；浏览器保存并在后续请求重放完整输出项和加密 reasoning，刷新不自动发请求。保存的原生输出必须与规范化历史中的文本及工具批次一致，不能通过篡改恢复元数据改变待执行工具。

Responses 可选接口默认、关闭、低、中、高、超高（`xhigh`）思考强度；接口默认不发送 `reasoning`，其余只发送显式 `reasoning.effort`。这些是协议选项，不代表所选模型都支持；不支持时显示接口错误，不自动降档或重试。可读 reasoning summary 流式显示为折叠思考区域，加密内容不显示。`response.incomplete/failed`、缺少完成事件、未知工具、无效 JSON 或不一致的输出项会阻止整批待执行工具。参考 [OpenAI 官方迁移指南](https://developers.openai.com/api/docs/guides/migrate-to-responses)。

服务需要允许页面 origin 的 CORS、POST 和 `Content-Type`。OpenAI-compatible 使用 `Authorization: Bearer ...`；Anthropic 使用 `x-api-key`、固定的 `anthropic-version: 2023-06-01`，直连另带 `anthropic-dangerous-direct-browser-access: true`，服务也必须允许这些请求头。HTTPS 页面不能请求被浏览器混合内容规则禁止的 HTTP endpoint。没有自动代理或 CORS 绕过。网络失败显示错误，不自动重试整个回合。

OpenAI-compatible 默认使用 `max_tokens`；服务要求 `max_completion_tokens` 时取消对应复选框。Anthropic 固定使用 `max_tokens`，隐藏不适用的参数开关。浏览器任务不再限制模型轮数：一直运行到模型完成、用户 Stop、请求/工具/存储错误或浏览器中断；不是后台常驻服务，原生 CLI 的轮次配置不变。底栏只显示已经进行的 step，不再显示 `/32`。输出 token 上限必须大于零。

模型名称可自由填写；官方 DeepSeek 的 OpenAI 协议连接提供 `deepseek-flash`、`deepseek-v4-pro` 参考候选。这是文档目录，不是实时查询的账号权限列表，也不代表第三方代理支持这些模型。模型连接页的 API 密钥下方集中显示思考模式、单次最大输出额度及 token 参数选项，位于“记住配置”上方；对上述已知模型提供“使用 DeepSeek 最大输出额度（393216）”按钮，并固定使用文档中的 `max_tokens` 参数。该操作不自动保存，也不静默覆盖旧的自定义额度；点击保存更改后生效。额度是上限，不是生成目标，不能绕过输入加输出的上下文限制。连接方式和流式选项也集中在模型连接页；自定义请求头折叠在“更多连接选项”内，桌面 CORS 扩展说明默认折叠，两者仅在直连时显示。Relay 模式显示访问令牌，不清空隐藏的直连配置。“不限制任务轮数”的运行说明移至工作区与恢复，高级页保留危险操作。

同一已知 DeepSeek 连接可选思考模式：接口默认、关闭、低、高、最高。接口默认不额外发送参数；其他档位明确发送 `thinking.type` 和 `reasoning_effort`，保存后用于后续请求，不中途修改运行中的请求。未知模型、其他供应商和 Anthropic 协议不会发送 DeepSeek 专用思考参数，也不会猜测最大额度。参考[官方参数说明](https://api-docs.deepseek.com/zh-cn/api/create-chat-completion/)和[思考模式](https://api-docs.deepseek.com/zh-cn/guides/thinking_mode/)；供应商能力变化后需更新目录。

首次打开且没有已保存连接时，会自动显示设置并聚焦 API URL，提示填写 URL、模型和 API key；默认勾选 **Remember credentials on this device**，但只有点击保存后才写入密钥。保存后自动关闭首次引导，刷新或重启浏览器可恢复；也可关闭引导稍后再配，或取消记住、仅在当前页面内存使用。Settings 按 Model connection、Workspace & recovery、Appearance、Advanced 分组；修改后点击 Save changes 应用。记住时，整套连接配置保存到当前 origin 的 localStorage `i_harness_profiles`：API URL、模型、API key、协议、思考档位、token 上限、token 参数选项、语言、transport、stream、relay token 和 custom headers，不写入 OPFS 工作区文件或会话数据库。旧 `i_harness_config` v1/v2 自动迁移，丢弃已移除的 max_steps，缺失思考档位补“接口默认”，保留原有密钥及显式 token 额度，不自动发请求或更换 key 接收方。连接测试和正式请求共用端点归一化及传输实现；测试会发小额模型请求。

首次界面语言按浏览器的 `navigator.languages` 偏好顺序匹配当前支持的语言；区域变体匹配基础语言，中文区分简繁（Hans/CN/SG 为简体，Hant/TW/HK/MO 为繁体）。没有可用匹配时才回退英语。已手动选择并保存的 `i_lang` 优先，其次保留已保存连接中的语言，不会被系统偏好覆盖。自动检测本身不写入密钥或触发模型请求。

新配置的最大输出 Token 默认值为 **65536（64K）**。已有连接保留其已保存的额度，不自动提高到 64K；复制连接仍沿用表单中的显式数值。这只是每次响应的上限，仍需符合所选模型或接口自身的限制，可在模型连接中调低。

API URL 未填写或格式不正确时，页面显示友好提示和“打开设置”按钮，定位到连接页的 URL 输入框，并提醒检查 API 密钥和模型；不会直接显示浏览器的 URL 构造异常，也不会回显含密钥的错误 URL。发送前先检查配置，保留未发送草稿；恢复队列遇到此问题时暂停并保留待发送消息。正确保存配置后清除该提示，不自动发送或重试。

旧会话中已经保存的特定 URL 构造错误，在恢复显示时提供历史错误说明和设置按钮；仅改变显示，不改写历史原文或删除文件，也不重新发送。普通模型回复即使包含相同文字，也不会被转换为设置操作。PWA 若仍运行旧版，请先应用更新，或关闭本站全部标签页及 PWA 窗口后重开，不要清除站点数据。

已安装的 Rust/WASM 插件不接收连接配置或 API/Relay 密钥：guest 模块禁止所有 imports，只能通过固定宿主操作访问授权工作区和隔离的插件 KV，不能直接调用 JavaScript、DOM、localStorage 或 IndexedDB；配置读取、网络请求和越界文件路径会被拒绝。同源页面脚本仍可读取本地存储，浏览器扩展和被篡改的宿主代码不属于这一 guest 隔离保证。不要主动把密钥粘贴进消息或工作区文件。

取消勾选会删除保存的连接凭据，当前页面的值仍可继续使用；Advanced → Danger zone → **Clear all credentials** 删除旧配置及所有 profiles，并清空活动凭据，但不删除会话历史或 OPFS 文件。删除连接后，旧会话不能静默使用其他连接；在设置中明确 Attach selected connection 才能换绑。修改已有会话的账号/租户凭据需确认。浏览器阻止存储、配额不足、JSON 损坏或版本不兼容时显示状态，不伪称保存成功。

localStorage 不是加密保险箱，同源脚本可以读取已保存 key，不应在共享设备上开启。清除站点数据也会删除配置。凭据不写 OPFS、会话或 ZIP；不要自行把密钥写入上传文件或任务文本。服务 URL 不允许内嵌用户名/密码、query 或 fragment。已有上下文切换 URL/model 时需要新建会话或显式换绑。

界面语言选择立即生效，不要求先填写连接；仅语言偏好独立保存为 `i_lang`，不因此保存 API key。清除凭据保留语言。设置页与主要控件提供中英文文案，模型输出、文件名、连接名称和上游技术错误保留原文；其他语言未补齐的新文案回退为英文。

“设置 → 外观”可选自动（跟随系统）、白天、暗色，立即生效；主题独立保存为 localStorage `i_harness_theme`，不依赖连接配置或保存 API 密钥。自动模式随系统主题变化，手动模式不随系统变化；其他同源窗口的主题选择也会同步。主题覆盖界面、文件树、队列、审批及源码配色，不改写用户 HTML/预览网页自身的样式。浏览器阻止存储时只在当前页面生效并显示未保存提示。

AI 运行时可在输入框继续编辑消息并点击“追加”，待发送列表支持编辑、取消和暂停/继续。当前任务成功后按顺序发送；停止、错误或刷新会暂停，需手动继续。队列归属于会话，切换标签页不转移消息；发送队列不清空正在输入的其他草稿。详见[输入与待发送队列](composer.md)。

默认开启 Stream，首段到达即显示；不兼容 SSE 的接口可手动关闭后重试，不自动重复请求。完整结束标记、结束原因与参数通过校验前不执行任何工具；取消或断流保留文字并标记 incomplete，但不把半截回复作为完成历史。模型请求不再设 3 分钟或 30 分钟总时长上限：连续 30 分钟没有响应数据才中止，收到数据后重新计时；等待首个响应同样受空闲期限限制。接口、反向代理和浏览器后台调度仍可能提前断开。默认 ask 模式下 write/edit 需要点击 Allow；Deny 会作为工具错误返回模型。Stop 中止请求及后续工具，已完成的文件写入不回滚。New conversation 新开独立模型历史而不删除旧会话或文件；Clear 只清显示。

流式接口返回可读 `reasoning_content`（OpenAI 兼容）或 `thinking`（Anthropic）时，终端显示默认折叠的“思考中…”区域，点击展开后实时更新，完成后保留为“思考内容”，与正式回复分开。只按纯文本展示，不展示签名、隐藏思考块或未完成的工具参数。持久会话保存这些已脱敏文本及展开状态；显示“已保存”后刷新可恢复。接口未返回可读思考内容时不创建空区域。

OpenAI-compatible 流的异常结束会显示实际 `finish_reason`：`length` 提示检查输出/context 限制或拆分任务，`insufficient_system_resource` 提示上游资源不足，`aborted` 提示上游中断，`content_filter` 提示过滤终止。其他原因明确报告不兼容，而不是统一隐藏为“unsupported finish_reason”。这些响应都不放行当前待执行工具，不自动增加 token 或重试；此前已提交的文件操作不回滚。DeepSeek 的结束原因含义见[官方 Chat Completions 文档](https://api-docs.deepseek.com/api/create-chat-completion/)。

### 执行模式

输入框下方底栏的执行模式与 `/mode` 命令共用同一会话状态：`/mode` 或 `/mode status` 查看当前模式，`/mode ask` 修改前询问，`/mode auto` 自动批准允许的文件修改。auto 使用醒目颜色，桌面显示“文件修改免确认”提示；手机优先显示模式及保存状态。输入框内按 Shift+Tab 可切换（候选菜单展开或输入法组词时不拦截）。新会话及旧数据默认 ask；持久会话记住模式，临时会话不落盘。选择器或快捷键切到 auto 会明确确认，命令中的显式 auto 参数直接生效。

模式属于会话而非全局设置，切换标签页时同步显示。运行中可用选择器调整后续审批，但已有待审批请求仍需 Allow/Deny，不会因切到 auto 被自动放行。模式不能开放 shell、绕过工具批次校验或启用预览 JavaScript，存储失败仍阻止后续工具。

### 斜杠命令和会话标题

在输入框开头输入 `/` 即显示本端可用命令和说明，继续输入按前缀筛选。上下箭头选择，Tab 或 Enter 填入候选，不自动执行；菜单关闭后 Enter 才提交。Esc 关闭菜单，手机可点选，输入法组词期间不拦截。完整键入的命令不再显示自身候选，可直接提交。`/mode ` 补全参数，`/resume ` 补全当前工作区真实会话 ID（说明显示名称），`/download ` 补全真实路径；`/download PATH` 只导出指定路径。任务中间出现的斜杠不触发。

`web/commands.json` 是网页、原生主控制台和旧 readline 入口的共享命令目录，按平台过滤；帮助和候选共用目录。浏览器不展示未实现的 `/compact` 或原生审批指令。该非敏感静态目录进入 PWA 校验缓存，不包含用户会话或文件内容。

新会话先显示“新会话”，首次非命令任务取第一句作为标题，移除简单 Markdown、折叠空白并截为约 24 字符，完整首句可悬停查看；后续普通任务不反复改名。双击标签页可手动编辑并锁定，刷新恢复后也不会被自动标题覆盖。旧自定义名称保守视为手动标题，旧 `Conversation N` 可从已有首个任务迁移。

标题提取支持已提交历史中的 `Session summary (untrusted historical context):` 压缩摘要，识别“当前目标/意图”或 `Current goal/Intent` 等明确段落，没有明确意图则保留原标题；未完成工具日志不触发，不额外发模型请求。**浏览器目前没有自动 compact 功能，本次仅接入摘要标题更新逻辑，不新增隐式压缩请求。**

## PWA、跨域与 PC 插件

默认 Direct 模式直接访问支持 CORS 的模型接口，不需要服务器或插件。PWA 不解除 CORS；网络 TypeError 可能是 CORS、网络或地址问题，需查看浏览器开发者工具确认，不能据此判断 key 无效。桌面 Chrome/Edge 用户可自行安装可信 CORS 插件，并将权限限制为当前页面和选定 API，测试完关闭；本页面不静默安装插件、不关闭全局安全设置。iPhone 不依赖桌面插件。

不支持直连的接口可显式选择 Own same-origin relay，并部署[独立可选服务](../server/README.md)。模型 key 会经过该自有服务，relay token 只发送页面同源 API，不支持输入任意第三方代理地址。直连失败不自动切换，不部署 relay 也不影响静态应用使用。

中继支持精确白名单中的 `/chat/completions`、`/responses` 与 `/v1/messages` 端点，按端点分离 Bearer 和 Anthropic 密钥，Anthropic 浏览器直连请求头不转发给上游。页面仍使用同源 `/api/chat/completions` 网关路径，协议与原生请求体不会转换成另一种模型 API。

iPhone：用 Safari 打开可访问的 HTTPS 地址，分享 → 添加到主屏幕。其他浏览器支持安装提示时显示 Install app，否则使用手动指南。第一次成功在线缓存后，离线可启动 WASM、访问 OPFS、上传与导出 ZIP；AI 仍需联网。不要假设安装态与普通标签页共享配置/文件，首次打开以实际状态为准。锁屏或后台可能暂停请求，不承诺持续生成。

Service Worker 只缓存构建白名单静态资源，逐项校验 SHA-256，不缓存模型 POST、响应、配置或密钥。更新提示出现后，用户点击 Apply app update；所有标签页空闲并确认锁定后才激活和刷新，生成/审批中延后更新。缺资源或版本不一致时安装失败，旧版本继续可用。同源根路径与子路径同时安装等情况下，浏览器可能不向等待 worker 递送消息；页面会明确显示 postponed，按提示关闭所有应用标签页/窗口后重新打开，不伪称已更新。不同注册不会互相锁住 UI。只清理本应用 scope 的旧缓存，不删 OPFS 或配置；浏览器回收/清站点数据仍可能删文件，应定期导出 ZIP。

更新检查最多等待 30 秒，让较慢的状态保存完成。运行任务、保存失败、页面未响应和检查期间打开新页面会显示不同原因；手机也会显示需要处理的提示。运行任务时不会强制刷新，可等待完成或自行停止后重试。未响应时先切换到其他应用标签页或 PWA 窗口唤醒它，或关闭空闲页面后重开；**不要清除网站数据**。更新只保留已保存的持久工作区、会话与配置，`/tmp` 和临时工作区仍须在更新前导出。旧版页面使用较短的锁定期限时，检查会在期限内安全延后，而不是把等待时间延长后误中断旧页面；关闭并重开空闲的旧页面即可使用新机制。

## 文件与下载

默认工作区保留 OPFS 的旧 `workspace/` 根；新工作区独立使用 `harness/workspaces/<UUID>/`，显示名不参与路径。顶部选择工作区，每个 Tab 是独立会话，同工作区会话共享文件，不同工作区文件隔离。旧 `i_workspaces` 仅为共享目录上的标签，迁移只映射默认目录名称并保留原始标签，不虚构文件隔离。支持二进制上传、嵌套目录、Unicode 名称，以及 glob/search/read/write/edit/view_image/list。优先搜索定位，再用 `read.files` 批量读取相关区间、`edit.edits` 批量修改同一文件；不保留旧参数接口。读取/搜索/目录工具默认共享 12000 字节输出预算，截断时返回续查位置。详见[文件工具接口与限制](file-tools.md)。浏览器 shell 在 schema、输入和执行层均禁用。

文件读写每文件最多 8 MiB，图片最多 8 MiB，每批最多 8 张；工具文本前缀预算 1 MiB。已知轻微差异：截断标记还会增加至多 12 字节。模型 HTTP 请求/响应上限 48 MiB。

Upload 写入成功并关闭 writable 后才报告成功；默认 ask 模式下重名需确认，auto 可自动批准覆盖。OPFS 不可用、配额不足或写入失败不会回退为假持久化内存文件。刷新后文件、会话上下文、终端文本、草稿、Tab 和当前选择从 OPFS/IndexedDB `harness_state` 恢复，不自动请求模型、重放工具或恢复审批。草稿/阅读位置/流式显示合并保存间隔 150ms；强制退出可能丢失最后未提交片段。工具执行前后分别等待事务完成；已开始但结果未提交的工具标记 outcome unknown，文件不会回滚，也不会自动重做。保存失败阻止后续副作用，仍允许只读 ZIP 备份。跨窗口使用工作区 Web Lock 和会话 revision 拒绝旧快照覆盖。

文件侧栏按 `/workspace` 和 `/tmp` 两个根显示可折叠目录树，文件夹优先排序，同级只显示图标与名称，选中后显示路径和字节数。上传目标、上传及 `/tmp` 操作收进“文件操作”，有下载产物时才显示下载记录。手机使用抽屉，选中文件后收起；预览中的“文件”按钮返回目录树。

根目录旁显示真实文件数量，折叠时也保留计数；普通 `/workspace` 标“刷新保留”，`/tmp` 与临时工作区标“临时文件”，不是整个 OPFS 会随刷新清空。“设置 → 工作区与恢复”显示当前数据所属 origin 和工作区。若怀疑文件消失，先核对地址（主机和端口）、当前工作区与 `/workspace` 计数并展开目录，不要清除站点数据。

持久工作区的目录展开状态、选中文件和预览模式存入工作区元数据，刷新恢复；临时工作区完全不保存，`/tmp` 展开项与所选临时文件也不持久化。

OPFS 存储层统一使用工作区级多读/单写锁，编辑、目录遍历、ZIP 和预览快照覆盖完整操作；排队写入可取消，已开始的 I/O 收尾后才释放锁。保证范围、显式作用域示例及跨 IndexedDB/多文件提交的限制见 [OPFS 并发](opfs-concurrency.md)。

同一独占事务内的并发文件操作也按调用顺序排队，避免多个 writable 在同一文件上后提交覆盖新内容。单文件在 `write` 和 `close` 完成后才报告成功，不等整轮模型任务结束；工具返回后合并刷新文件树，因此模型继续思考时也能看到已提交文件。失败时中止 writable，保留已有文件；仅清理本次新建且尚未成功提交的占位文件。突然关闭页面仍可能留下未完成的新文件或目录，多文件事务不保证全有或全无；没有把会话保存状态当作文件持久化证明。回归测试从真实 WASM 工具写入，等待下一条模型响应期间验证目录显示，并连续刷新三次直接读 OPFS 校验内容。

Clear 仅清终端显示；Reset conversation 需确认并清当前模型上下文，保留文件；New conversation 新开独立上下文；关闭 Tab 不删除历史，History 或 `/resume` 可重开。Workspace & recovery 可单独删除所有会话，不清文件或凭据。ZIP 只备份工作区文件，不含会话数据库。存储按 origin 隔离：更换协议/主机/端口会看到不同数据；清除站点数据或浏览器回收存储会删数据，应及时备份。

### 临时工作区和 /tmp

顶部 **Scratch** 创建临时工作区，标题始终标记 Temporary。它的文件、会话、草稿仅在当前页面内存中，不进入 OPFS、IndexedDB 或恢复 Tab 元数据；刷新、关闭页面或 PWA 更新都会丢弃，需提前下载 ZIP。

每个工作区另有独立的 `/tmp` 内存挂载，供同工作区各会话共享；切换会话保留、刷新清空。`/workspace/a.txt` 是普通相对路径 `a.txt` 的别名；`/tmp/a.txt` 是临时文件，**相对路径 `tmp/a.txt` 仍是普通工作区文件**，不会被挂载遮蔽。仅浏览器允许这两个绝对虚拟路径，不放宽 Native 的工作区路径保护。

文件栏 Upload destination 选择 `/workspace` 或 `/tmp`；Clear /tmp 确认后只清当前工作区临时目录；Download /tmp ZIP 单独备份。默认 Download ZIP / `download({})` 不包含 `/tmp`；模型可显式 `download({"paths":["/tmp"],"filename":"tmp.zip"})` 导出，ZIP 内为相对的 `tmp/...` 路径。混合选择普通 `tmp/a` 与 `/tmp/a` 导致条目重名时明确报错，不覆盖。

单文件沿用 8 MiB 限制。持久工作区的 `/tmp` 总计最多 32 MiB；临时工作区的普通根与 `/tmp` **共同**使用 32 MiB 配额。清 `/tmp` 只释放其占用，不重置普通根已用额度。内存存储不是浏览器 OPFS 持久存储的备用假实现。

### 文件预览与 HTML 快照

点击文件可切换 **源码 / 预览 / 分栏**。源码为只读 UTF-8 文本；代码和普通文本安全显示，JSON 预览格式化但源码保留原文。Markdown 提供标题、加粗、行内代码、代码块与简单列表的基础渲染，原始 HTML 转义、链接保持文本，不加载外部图片或脚本；不是完整 CommonMark 实现。图片可预览，未知二进制明确提示不支持源码，不尝试执行。分栏在桌面左右排列，手机上下排列并允许滚动。

单文件读取保持 8 MiB 限制，所有读取使用工作区锁。预览状态恢复不会发送模型请求或恢复脚本授权；切换文件或预览模式也会清除脚本开关，需要重新确认。

如果恢复的预览文件不存在，只在预览区显示明确路径及“可能尚未创建或已被移走”的提示，不将可预期的缺失文件报成全局初始化故障。不创建空文件，也不删除文件；失败预览不再作为已打开预览保存，下次刷新不会重复自动打开它。

点击 **Preview HTML** 默认打开 `index.html`；也可在文件列表点 HTML 的 Preview，或输入 `/tmp/index.html`。预览读取所选工作区/挂载的只读快照，桌面右侧显示、手机覆盖主区域；Refresh preview 重新取文件，关闭或切换工作区销毁旧预览。不是实际 HTTP 服务，不运行 Node/PHP/Python 后端。

默认禁止脚本，可预览本地 HTML、CSS（含普通 `@import` / `url()`）和图片。勾选 **Run JavaScript (trusted files only)** 并确认后才运行普通内联/外部脚本及本地 ES Module。本地 `fetch()` 被模拟为快照 GET/HEAD；相对路径、根路径和查询参数可用，缺文件返回 404。脚本模式能读取该挂载快照中的所有文件，只应对可信代码开启。

iframe 使用独立 opaque origin，不授予 `allow-same-origin`、forms、popups、下载或顶层导航权限。子文档 CSP 限制普通外部资源/网络，父页面 `frame-src about:` 阻止预览自行导航到 HTTP 页面。Harness 配置、API key、会话数据库从不传入预览。**CSP 不等于完整网络沙箱**，尤其不能保证可信脚本模式下所有浏览器特有通信通道都被禁用；不要将此模式用于执行敌对代码。

脚本模式为预览页提供独立的内存 `localStorage/sessionStorage`，支持常用 Storage 方法和命名属性；两者共享 1 MiB UTF-16 配额，超额写入抛出 `QuotaExceededError`，不覆盖已有值。它们不是工作台的真实浏览器存储，不读取或写入父页面配置，不支持跨窗口 storage 事件或 IndexedDB。刷新/关闭预览会清空，仅影响预览临时状态，不删除 OPFS 文件、会话或密钥。纯预览模式不再构建隐藏源码高亮；超过 128 KiB 的源码按完整纯文本显示，减少大量高亮节点带来的卡顿。

本地 ES Module 支持内联或 `src` 入口、相对/根路径 `import`、`export ... from`、默认/命名导出、循环依赖、共享模块状态、顶层 await，以及固定字符串的动态 `import('./lib.js')`。`import.meta` 提供虚拟工作区 `url`（不是页面真实地址），可通过 `fetch(new URL('./data.json', import.meta.url))` 读取本地数据；不提供 `import.meta.resolve` 等其他能力。模块图由随站点缓存的 es-module-lexer 1.7.0 定位依赖，再通过 import map 交给浏览器原生模块引擎运行，不使用正则转译 import/export，不依赖运行时 Node 或 CDN。第三方原始文件、许可证与校验值见 `web/module-lexer.*`。

不支持 npm 裸包名、外部 CDN、计算式动态导入、自定义 import map、JSON/CSS 模块导入属性或 source/defer 阶段导入；需要先将这类项目构建为本地 JavaScript 文件。缺失模块或不支持的导入在执行脚本前报错，中文界面显示中文原因。关闭脚本时不解析或加载模块依赖。模块预加载提示移除，依赖从本次快照提供；模块 URL（含查询参数）最多 1000 个，展开后的模块资源最多 48 MiB。

不支持动态资源路由、XHR、本地服务端 API、跨页面链接跳转或外部 CDN。未找到资源、CSS URL 转义、过深导入会明确报错，不虚构运行成功。静态 `srcset` 被移除，保留 `src` 资源。快照上限 16 MiB / 1000 条目，展开后 HTML 上限 48 MiB；预览读取与文件写入共用工作区锁。快照不进入 PWA Cache Storage 或数据库，离线也可重新预览已保存文件及本地模块。

按钮 Download 与模型工具 `download({paths?, filename?})` 共用 Rust ZIP 实现。默认递归导出全部 workspace；paths 可选择文件或目录；filename 默认 `workspace.zip`，必须是无路径的 `.zip` 名称。保留目录、空目录和二进制字节；stored ZIP 不压缩内容。单次最多 64 MiB 文件内容、10000 个条目，超限失败而不是生成截断 ZIP。

模型调用 download 会生成持久可点击的 Save 链接，用户点击后保存；工具的「ZIP ready」不代表文件已经存入本机。按钮也保留该链接，便于浏览器拦截自动下载时手动保存。ZIP 默认不含终端显示、配置或 API key，只含用户工作区文件。

原生 download 默认写 `.i/downloads/`，默认全工作区导出排除该自动产物目录；覆盖必须确认。输出不得与显式源路径重合，拒绝符号链接输出及其符号链接祖先，并使用同目录临时文件原子替换，避免写穿硬链接源文件。

## 验证与边界

浏览器回归覆盖会话模式隔离与恢复、auto 不绕过旧审批或禁用工具、文件树与挂载隔离、手机抽屉返回、源码/渲染/分栏、JSON/图片/二进制、Markdown 转义、临时预览状态不落盘、斜杠菜单键盘/手机/输入法交互、真实路径与会话补全执行、底栏快捷键、首句标题及手动锁定、旧标题脱敏迁移及其持久化。测试数量会随版本变化，请以当前提交的测试输出为准。

```sh
cargo build -p i-plugins
cargo test --workspace
cargo check --lib --target wasm32-unknown-unknown --no-default-features --features wasm
./scripts/build_web.sh
npm --prefix web ci
npm --prefix web exec -- playwright install chromium webkit
npm --prefix web test
sh scripts/build_plugin_fixtures.sh
npm --prefix web run test:browser
cargo test --locked -p i-relay
```

Web 测试启动 localhost 静态服务与 mock API，不依赖付费模型。覆盖模型工具审批/拒绝、OPFS 持久化及隔离、会话 Tab/历史/草稿恢复、连接换绑、损坏会话隔离、事务失败/多窗口冲突、未知工具结果恢复、保存失败仍可备份、二进制上传、ZIP 独立 unzip 解压、取消晚到响应、IME、截断批次、CORS 失败、OPFS 不可用以及 PWA 离线/更新。另覆盖临时工作区不落盘、/tmp 生命周期、聚合内存配额及清理、虚拟挂载 ZIP 重名拒绝、真实 WASM 工具临时写入、预览 HTML/CSS/脚本/fetch、默认脚本禁用/可信确认、父存储隔离、HTTP 外跳阻止、挂载快照隔离和工作区锁争用。Anthropic 测试覆盖原生鉴权、流式/非流式工具循环、thinking/signature 续聊、空参数工具、初始文本与多文本块、截断拒绝、混合 shell 批次拒绝、中继、刷新无请求和协议切换元数据隔离；设置测试覆盖桌面控件对齐、手机宽度、中英文切换及输入节点保留、语言独立持久化、API URL 归一化和测试失败文案。桌面 WebKit 与手机 viewport 不等同于真实 iPhone 实测。

发布快照已修复原生网络重连、显式直连覆盖环境代理、代理协议校验及剩余请求期限问题；`tests/network.rs` 的 10 项回归通过。TLS 本地自签名证书测试要求实际 ClientHello、拒绝不受信任证书且不重试，不能仅凭 EOF 判定通过。旧 `all_core_production_rust_stays_under_1024_lines` 保留为显式 ignored 的历史约束；共享核心/WASM 不再以 1024 行为目标，不通过挪文件隐藏代码。

源码统计（包含空行/注释与文件内测试，排除独立 `src/tests.rs`，包含 packaging/completions）：

```sh
find src -name '*.rs' ! -path src/tests.rs -print0 | xargs -0 wc -l
```

源码数量会随实现变化，以上命令可复算；另有 `plugins/runtime`、vendor、JS、JSON、测试等，不能把 src 行数当完整客户端总量。旧原生发布体积测量不适用于当前重构；本次未重测原生发布体积。测试不使用真实付费模型；桌面 WebKit/手机 viewport 测试不等于真实 iPhone、Safari 安装流程或 Windows 实机验证。

参考：[MDN OPFS 入口与安全上下文](https://developer.mozilla.org/en-US/docs/Web/API/StorageManager/getDirectory)、[wasm-pack Web 构建](https://wasm-bindgen.github.io/wasm-pack/book/commands/build.html)、[Anthropic Messages 流式协议](https://platform.claude.com/docs/en/build-with-claude/streaming)。
