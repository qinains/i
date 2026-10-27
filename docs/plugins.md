# 插件、worker 与平台契约

本页主要描述**原生桌面 NDJSON worker**。浏览器 Rust/WASM 工具插件采用另一套受限协议：
先阅读 [插件开发教程](plugin-development.md) 和 [WASM 插件参考](wasm-plugin-reference.md)。
浏览器 CORS 扩展也不是 worker 或 `.hplugin.zip` 插件。

原生插件使用单一新版协议，没有旧进程协议、旧目录发现或迁移兼容层。可选 Rust 包位于 `plugins/runtime/`。默认独立核心不链接该包；`bundled` feature 将它链接进同一个发布程序，通过 `i --runtime` 启动前端/独立 worker 进程，共享二进制依赖但不改变 worker 协议。核心传输/重试位于 `src/worker.rs`、`src/net.rs`。关闭 feature 后可配置外部 provider worker，或旁置 i-plugins；调用模型不依赖核心内置的某家协议。provider 架构见 [providers.md](providers.md)。浏览器 WASM 插件采用另一套隔离协议，参见 [WASM 插件参考](wasm-plugin-reference.md)。

## 1. 桌面清单

```json
{
  "tools": [
    {
      "name": "my_tool",
      "description": "Describe the capability and side effects",
      "parameters": {"type":"object","properties":{"text":{"type":"string"}},"required":["text"]},
      "exec": ["/absolute/path/to/worker", "mode"],
      "cwd": "/absolute/path/to/config"
    }
  ],
  "hooks": [
    {"events":["before_turn"],"exec":["/absolute/path/to/worker","mode"]}
  ]
}
```

- `exec` 是 argv 数组，不经 Shell 拼接。可执行名没有路径分隔符时按 PATH 查找，否则相对最终 cwd 解析。
- `cwd` 默认是清单目录；相对 cwd 也相对清单目录。最终目录必须存在并可规范化。
- 工具名1–64个 ASCII 字母/数字/`-`/`_`，不得重复或覆盖内置工具；参数 schema 必须为 object。
- hook 按清单顺序串联，前一个返回的完整 `data` 作为下一个输入；事件可为 `*`。
- 传给模型的 schema 只有公开名称、说明和参数，不包含 exec/cwd。
- 注册是信任决定。hook 自动执行；每个外部工具调用均需审批。

bundled 的 `i --runtime init` 创建 `config/plugins.json`（可移植选择）、`config/bindings.json`（本机 worker 替换）及 `config/builtin.json`（动态内置描述符）。独立 `i-plugins init` 保留供 core-only 使用的 `desktop.json` 静态清单。可移植选择格式：

```json
{"profile":"standard","disabled":["web"]}
```

`standard` 来自 `full` 但去掉 Responses 转换；还支持 `approval`、`budget`、`compact`、`reflection`、`cache`、`responses`、`responses-budget`、`responses-compact`、`memory`、`search`、`skills`、`web`。`budget` / `responses-budget` 只计量预算，`compact` / `responses-compact` 另外启用压缩。`reflection` 是显式对话分析入口；`standard/full` 注册该入口但不自动调用模型，见 [对话分析](conversation-analysis.md)。

`glob/search` 已改为跨平台内置工具；`full/standard` 不再重复注册外部 `search/find_files`，`search` profile 不额外注册工具。旧静态清单需重新生成，自定义同名条目需移除；新的 `search` 用 `pattern`，批量读写接口见[文件工具](file-tools.md)。

`i --runtime manifest PROFILE` 使用当前程序绝对路径生成清单，并应用 `bindings.json` 中的模块替换，例如 `{"compact":{"exec":["/absolute/custom-worker"],"cwd":"/absolute/config"}}`。替换会覆盖该模块全部已选 hook/工具/analysis 入口，不改变工具公开 schema；失败不会回退内置实现。`disabled` 同样作用于 analyses。生成项的 `module` 仅用于标识，不发送给模型。该机制不替换核心原生工具或安全校验。

新 `builtin.json` 使用 `{"builtin_profile":"configured","tools":[]}`，每次启动根据当前程序路径和选择/替换配置动态解析，无需保存开发目录路径。只有旧式/静态清单在程序路径或选择改变后需重新生成；初始化不覆盖已有文件。发布包启动器使用全局配置，开发启动器仍生成临时清单。模型/服务地址/预算位于 `config.json`，凭据只存引用，不存 secret；首次配置与旧清单迁移见 [deployment.md](deployment.md)。完整替换契约与 Rust 示例见 [compaction.md](compaction.md)。

清单顶层可包含 `provider: {exec, cwd}`；生成器应用 `bindings.json` 的 provider 绑定。它不是普通 hook，不发送给模型。API 配置中的显式 provider 绑定优先。详见 [providers.md](providers.md)。

## 2. 常驻 NDJSON worker

同一核心进程中，相同 argv 和规范化 cwd 共享一个 worker，顺序调用，不并发复用同一输入流。工作区和桌面环境在该核心进程生命周期内固定。进程退出或失败不会被透明重放；后续显式调用可以新建 worker。

每个请求是**一行 UTF-8 JSON，以换行结束**：

```json
{"id":1,"event":"tool","workspace":"/project","data":{"name":"my_tool","arguments":{"text":"hello"}}}
```

成功响应：

```json
{"id":1,"data":{"output":"result text"}}
```

失败响应：

```json
{"id":1,"error":"failure description"}
```

规则：

1. ID 为无符号整数，响应必须回显同一 ID；ID 只在当前 worker 内关联请求，不是持久全局 ID。
2. 成功 `data` 必须为 object。hook 返回完整数据，不是补丁；不处理的事件通常原样返回 `data`。
3. 必须每帧 flush；stdout 只能包含协议帧，不能输出启动 banner 或日志。
4. stderr 可用于诊断，但核心只做有界消费，不保证回显内容；累计字节受 `io_limit_bytes` 限制。可预期错误应使用错误帧。
5. 单帧最多48 MiB（包含换行）；输入、输出都检查。缺少换行、EOF、无效 JSON、错 ID、非对象 data、错误帧均视为失败。
6. 核心每次调用15秒，包含序列化、阻塞写入与等待响应。超时或错误逐出 worker 并清理进程组；退出正常核心进程时也清理池。
7. **不自动重试失败调用。** 返回错误时工具可能已经产生副作用；批次不是事务，磁盘会话仍保留上一个完整检查点，不回滚已执行的操作。

池键当前为 argv + canonical cwd，不会根据不同 JSON 格式自动猜测协议，也不是跨 CLI 进程的 daemon。预算 worker 不再打包本地 BPE tokenizer 或词表。仍有 JSON 序列化、复制和 IPC；没有实测吞吐提升数据。

环境默认清空，只保留 PATH、固定 `PYTHONUTF8=1`、存在的 SystemRoot/TEMP/TMP/HOME/USERPROFILE，以及四个 `I_*_DIR`。不会自动把 API_KEY、自定义模型凭据变量或 `I_APPROVAL_TOKEN` 传给插件。**这不是恶意插件隔离**：可信程序仍有当前用户的文件/系统权限。Shell 是独立内置工具，继承普通环境，但去掉审批 token。

示例实现：`examples/rust-plugin/src/main.rs`。离线协议 fixture 位于 `tools/test-fixtures/`，不是生产兼容层。

## 3. 路径上下文与工作区身份

官方 worker 另外接受顶层平台上下文：

```json
{
  "id": 2,
  "event": "tool",
  "workspace": "/authorized/project",
  "workspace_id": "f29127b0-f512-4c6e-9e31-87d476d168ce",
  "paths": {
    "config_dir": "/app/config",
    "data_dir": "/app/data",
    "cache_dir": "/app/cache",
    "runtime_dir": "/app/runtime"
  },
  "data": {"name":"memory_get","arguments":{}}
}
```

- `paths` 存在时不依赖 HOME；四个字段都必须是绝对路径，不允许 `..`。不允许 config/data 位于 cache/runtime 下，校验当前可解析的别名。文件系统变化竞态不属于隔离保证。
- 桌面未提供上下文时，分别用 `I_CONFIG_DIR` / `I_DATA_DIR` / `I_CACHE_DIR` / `I_RUNTIME_DIR` 覆盖 `~/.i/{config,data,cache,runtime}`；Windows 的 HOME 默认源为 USERPROFILE。
- 核心桌面适配目前通过环境传递目录，不能从模型参数中改变顶层 workspace/paths。显式请求上下文供其他宿主使用。
- 未指定 ID 时，桌面用规范化工作区路径的摘要查 `data/workspaces.json`，分配 UUID。注册表有锁、原子写入，最多10000条、读取上限2 MB；已有绑定不会重复重写注册表。
- 指定 ID 必须是 UUID，视为可信宿主提供的绑定。它不是权限授予；移动端仍需验证文档访问 grant、安全作用域和工作区归属。
- 持久工作区状态放在 `data/workspaces/<id>/`，其中 memory 为 `memory.json`，缓存统计为 `metrics/cache-stats.json`。配置不是这些目录中的项目文件，指标也不是可删除 KV 缓存。
- 控制台把这里的 `sessions/<name>.json` 显式交给核心。核心独立 `--session-name` 管理的是项目 `.i/sessions`；两个入口不共享命名空间。核心 `--session FILE` 是宿主无关的存储入口，会话仍校验 workspace。

`i --runtime paths -C PROJECT` 可查看解析结果。改路径、项目移动、跨设备同步不是自动迁移。当前只实现桌面 subprocess 后端；没有移动端 UI、静态插件执行后端、移动端 FFI 或远程执行服务器。

## 4. 配置发现

官方模块优先使用显式 `--config FILE`，否则发现 `config_dir/<module>.json`。显式参数路径按 worker cwd 解析；配置中的相对文件路径按各模块契约解析，通常相对配置文件。具体例外是 cache 的 `stats`，它必须相对工作区**状态根目录**。

- `approval.json`：可选 `mode: ask|auto`、`deny_tools`；可替换审批协议与控制台模式见 [approval.md](approval.md)。
- `cache.json`：mode / target / namespace / stats 等。DeepSeek 建议 observe；前缀稳定性检查、实际用量校验及报告字段见 [cache.md](cache.md)。
- `web.json`：服务地址、GET/POST、参数映射、凭据文件、私网访问选项。
- `skills.json`：`{"roots":["/absolute/root","relative/root"]}`，相对根相对配置文件。
- `prompts.json`：默认清单用 `{"system":"prompts/system.txt"}` 选择 system 提示词；手工注册 `before_compact` 的 prompts hook 仍支持其 `compact` 字段。
- `provider.json`：默认 `{"images":"base64"}`；DeepSeek Files 可显式配置 images/files、account_scope、expires_after_seconds，并需 API 配置 provider_operations=true 双重授权。恢复与本地注册表命令见 [provider-files.md](provider-files.md)；远端管理使用独立 [--provider-action 通道](provider-files-management.md)，普通 DELETE 强制确认且不重试。另有独立授权的 [自动刷新/垃圾回收](files-lifecycle.md)，不会由 yolo 隐式开启。
- `compact.json`：`{"enabled":true,"keep_recent":4,"images":"keep","prompt":"prompts/compact.txt"}`，默认压缩策略的独立配置；相对提示词路径按配置文件解析。
- 预算参数来自核心 API 配置中的 `budget`，不是自动发现预算文件就能改变核心声明的模型容量。

模型 JSON 使用内置 selector `protocol: openai|deepseek|anthropic|responses`（自定义 worker 可解释其他字符串）、可选 `provider` worker 绑定、默认关闭的 `provider_operations` 同源附加请求授权、`base_url`、`model`、`api_key_env`、`max_tokens`、`use_max_tokens`、`plugins`、`io_limit_bytes`、`budget`、`extra_body` 等受校验字段。`use_max_tokens` 只为原生 Chat 选择服务商所需的输出参数字段。不要将 extra_body 用作凭据存储，内置 provider 不允许覆盖其保留控制字段；服务商字段验证由 provider 执行，核心继续验证通用传输与执行边界。

## 5. 工具结果和统一流程

工具请求中的 `data.arguments` 是解析后的 JSON object。核心 `before_tool` hook 接收的 `arguments` 则是原始 JSON 字符串，便于明确替换整次调用。

```json
{
  "id": 3,
  "data": {
    "output": "Image attached",
    "images": [
      {"type":"image_url","image_url":{"url":"data:image/png;base64,..."}}
    ]
  }
}
```

`output` 必须是字符串，`images` 可省略。外部图片与原生图片使用相同签名/大小检查：PNG/JPEG/GIF/WebP、每张8 MiB、每批最多8张，只接受 base64 data URL。结果先经过 `after_tool`，再检查图片并追加带工具调用 ID 标签的 user 图片消息。文本按动态工具预算和资源上限截断。

每批 tool_call 的 ID、名称、字符串 arguments、function 类型与结束原因先验证，再执行任何工具。JSON 参数语义错误会作为工具错误返回。所有工具经过核心审批门禁及可选 `tool_approval` 策略；默认内置 read/view_image 免询问，其他内置工具和所有外部工具需询问（显式 `--yolo` / `--dangerously-skip-permissions` 除外）。策略可返回 allow/ask/deny，非法值或失败不放行；编辑保留审批前内容快照并检查期间变化。不要把这一流程称为任意 JSON Schema 的完整验证器或执行沙箱。

## 6. 生命周期

| 事件 | 主要 data / 作用 |
|---|---|
| `session_load` | `messages`；准备本地会话元数据 |
| `system_prompt` | `prompt`；静态提示词选择 |
| `user_input` | `content`；用户输入处理 |
| `before_turn` | `messages`；安全边界 steering、参考资料更新 |
| `context_budget` | 模型、预算、历史、公开工具、待执行数、compact 标记；返回 fits/估算等 |
| `compact_plan` | 预算结果与完整 `history`；插件选择 `cut`（null 表示不压缩）及可选 `state` |
| `before_compact` | 已校验的旧前缀 `messages`、`cut`、`state`、模型与策略；返回 prompt/预处理文本/图片，或直接返回本地 summary |
| `after_compact` | `summary`、`state`；校验/调整摘要 |
| `before_compact_commit` | 恢复快照，再做最终预算校验与提交 |
| `before_request` | `url`、`body`、协议/模型/预算上下文，可附带 `request_meta` |
| `request_retry` | `request_meta`；每次物理重试前通知，返回值不修改请求，缓存模块增加 prepared 计数 |
| `after_response` | 原始 `response` 与对应 `request_meta` |
| `before_tool` | `name`、JSON字符串 `arguments` |
| `tool_approval` | `name`、对象 `arguments`、`external`、`decision: allow/ask/deny`；核心执行最终门禁，见 [approval.md](approval.md) |
| `after_tool` | `name`、`output`、可选 `images`，也覆盖工具错误 |
| `before_save` / `after_save` | `messages`、`path`；保存前校验完整历史，成功保存后确认 steering |
| `finish` | 完成后的 `messages` |

核心拒绝 hook 改变模型请求的 origin，不跟随模型 HTTP 重定向；但 hook 是可信代码，可以改变同 origin 请求内容。`request_meta` 不发送给模型。HTTP 成功/错误体最多48 MiB。

缓存观测响应应早于可能拒绝不完整响应的协议适配器；缓存请求准备应在 wire 内容变换后。仓库 `full.json` 已按此排序，包含 Responses；默认桌面 standard 去掉该转换。不得在缓存准备后悄悄改写影响前缀的内容而仍相信其预测分类。

## 7. 预算与压缩

`budget` 与 `compact` 是两个独立 worker，可分别替换。没有压缩插件时核心不自行选裁剪点或回退默认摘要；预算仍会在超限时阻止模型 HTTP。核心保留完整工具批次校验、模型调用、预算及事务提交职责。默认压缩器跟随预算建议，并至少保留最近4条消息，可通过独立配置调整。自定义插件可直接返回摘要，不必调用摘要模型。

预算策略通过配置提供容量、输出预留、阈值、安全比例、工具输出比例与图片规划值；核心不含模型容量表。默认触发 `min(window × .8, window − max_tokens − safety)`，默认 safety=.02、工具分配比例=.02。工具预算再按待执行调用数与剩余容量分配。

预算优先采用有效的输入＋输出 usage；锚点必须匹配模型和历史前缀，然后对新增内容和工具 schema 预留保守开销。本地消息顶层 `_` 元数据不参与指纹，但会影响 wire 重放的 `_responses_output` / `_anthropic_content` 参与。首次请求、模型/前缀改变、usage 缺失/不完整/非法/溢出时使用 UTF-8 字节估计，不把未知消耗当0。图片仍需显式 `image_tokens` 规划，因为上一轮 usage 不能预测新图片。

`tiktoken-rs` 及四份 BPE 词表已经移除，不能再选择 `cl100k_base` / `o200k_base` / `p50k_base` / `r50k_base`；显式配置这些值会失败，而非静默降级。`tokenizer` 可省略；显式 `utf8_bytes` 也表示同一个保守估计器。即使用真实 usage 作历史锚点，也不是下一轮服务端精确计数承诺。返回的输出 usage 可能含不全部重放的推理开销，因此整体仍可能高估。

摘要请求同样必须 fits，不计入未使用的工具 schema。压缩候选必须降低可比 token 估算并满足预算；失败不提交，原始历史/备份保留。不能靠字节减少冒充 token 减少。默认插件找不到安全旧前缀时不压缩；后续常规模型请求仍受预算限制。非法裁剪点、空/非法/不缩小摘要或超预算候选明确失败，未实现分块摘要。

`context.rs` 为 memory/skills 维护不可信 full snapshot 和 upsert/remove delta。仅追加，不改 system/历史前缀；内容不变不追加。压缩删除 baseline 时，`before_compact_commit` 先恢复最新 full snapshot，再由核心验证/预算/保存。

## 8. 缓存统计

```json
{"mode":"observe","namespace":"my-workload","stats":"metrics/cache-stats.json","target_hit_ratio":0.99}
```

- `observe` 不添加服务商缓存字段。
- `openai` 添加稳定64字符 `prompt_cache_key`，可选 `retention: in_memory|24h`；已有请求字段不覆盖。
- `anthropic` 添加顶层 ephemeral `cache_control`，可选 `ttl: 5m|1h`；已有字段不覆盖。
- `enabled:false` 可关闭；`stats` 相对工作区状态根，不能使用绝对路径或 `..`。
- 参数只在相应服务商支持时有效，选择模式是操作者责任。

```sh
i --runtime cache-report -C PROJECT [--config FILE]
```

统计公式为 `Σ cache-read input / Σ all input`。OpenAI/Responses 的 cached 已含在总输入；Anthropic 的普通、read、creation 相加得到总输入，creation 不是命中；DeepSeek 采用 hit/miss 分区。

`observed_token_hit_ratio` 基于已返回的可识别 usage；任何未知 usage 或准备后未观测到响应的请求，都使 `target_met:null`。准备不等于实际已发 HTTP。冷请求、miss、过期都保留，不能用相同前缀推定命中。

`new/append/reset/compact` 是预测分类；保留最近200条记录、256个 stream，累计计数保留。统计锁与原子更新，不存提示词、工具正文或凭据。默认99%是明确工作负载的目标，不是所有会话、单次请求、Windows/mobile 或线上已经达成的结果。

## 9. 搜索、网页、Skill、记忆与提示词

- 搜索返回路径、行号、片段；支持 glob/regex/case，最多遍历10000个文件、20000个条目，跳过隐藏/构建/符号链接/二进制及超大文件。
- 网页文本体上限1 MB、输出上限50000字符、最多5次 HTTP 请求、8秒客户端超时；校验公共地址/DNS/重定向，不用环境代理。默认拒绝私网；显式可信配置可例外。没有 JS 执行。
- 搜索 GET/POST 和 JSON 参数/结果映射可配置，凭据通过显式文件读取，不放进 prompt。见 `examples/web.config.json`。
- Skill 默认根 `.agents/skills`、`.i/skills`，只自动注入名称/描述/来源；外部根必须明确配置。每根最多100个 skill、1000个目录项，文件上限64000字节；name 为1–64 ASCII字母数字/`-`/`_`，description 最多1024字符。YAML metadata 类型受校验，重复名称报错。
- `skill_load` 读正文，`skill_read` 读相对引用且不能逃逸；脚本只作为资料，仍需正常工具审批才执行。
- 记忆工具都需外部工具审批；后台注入只在启用模块时发生。文件有锁和原子写入；不做向量检索，不存 secret。
- 提示词有嵌入默认值，替代文件须1–32000字符，支持 `{{workspace}}`。动态记忆/Skill 不拼回 system。

控制台的中文编辑使用系统终端接口和标准库，不新增 crate；输出统一绘制，非 TTY 保持按行模式。复杂 Unicode 编辑边界、清理与伪终端测试见 [console.md](console.md)。

## 10. 纯 UI 品牌与实测策略

`brand.rs` 根据共用字形生成终端/SVG Logo。`i --runtime logo --plain --ascii` 无 ANSI；`--svg` 输出向量文件；`NO_COLOR` 关闭颜色；控制台 `--no-logo` 完全关闭。默认清单不注册品牌模型工具，控制台显示不进入 prompt/schema/history。

`plugins/smoke.json` 是显式低成本连通性策略：移除工具、输出上限32 tokens、拒绝返回的工具调用，缓存只观察并写独立 `metrics/smoke-cache.json`。`examples/deepseek.smoke.json` 指向 DeepSeek 并只引用 `DEEPSEEK_TEST_API_KEY`，不存 key。真实请求仍计费，单次不能证明99%。smoke worker 支持显式 `--output-cap 1..1024`（默认32）；`plugins/smoke-thinking.json` 与 `examples/deepseek.thinking-vision.json` 提供1024上限的 thinking/看图测试配置，仍禁止工具。

核心、插件、前端、示例与测试分开计数；源码行数与当前验证结果以 README 为准。固定工作负载的延迟、吞吐与真实命中率尚未实测，不能把 Rust 或常驻进程本身当成性能结论。
