# i · lninl

[官网与文档](https://i.lninl.com/docs/) · [插件开发教程](docs/plugin-development.md) · [贡献与 PR](CONTRIBUTING.md) · [MIT](LICENSE)

![lninl：突出中间的 i](assets/lninl.svg)

**Small core. Your tools. Your model.**

插件优先的 Rust 编程 Agent，原生终端与浏览器 WASM 共用核心。内置工具和插件工具走同一执行流程；插件使用有边界、有超时的常驻 worker。扩展后的跨平台实现不再以历史的1024行作为目标。品牌 **lninl** 中间的 **i** 由代码绘制，只用于界面，不进入模型提示词。

文件工具优先减少模型轮次与上下文：内置 `glob/search` 定位，`read.files` 批量读取区间，`edit.edits` 单文件批量修改，默认紧凑输出并提供续查位置。不保留旧参数接口，详见[文件工具](docs/file-tools.md)。

Web/console 支持待发送图片与长文本折叠标记，图片可补充说明或单独发送；Web 草稿支持刷新恢复。操作方式与 console 剪贴板限制见[输入与粘贴](docs/composer.md)。

[English](README.md) · [插件与平台契约](docs/plugins.md)

## 当前能力

- [浏览器 WASM PWA](docs/wasm.md)：隔离工作区、独立会话 Tab、刷新恢复上下文/草稿、Chat Completions / Responses / Anthropic 流式请求、OPFS 文件、审批、ZIP 下载和离线启动；默认直连，缺少 CORS 时可选[自有同源转发](server/README.md)。浏览器禁用 shell/进程插件。执行 `./scripts/build_web.sh` 后通过 HTTPS 发布静态产物，localhost 用于开发。
  支持刷新即丢弃的 Scratch 工作区、独立 `/tmp` 和隔离 HTML 快照预览；脚本默认关闭，可信代码可确认开启，非完整后端服务器。

- 可替换的 OpenAI Chat/Responses、DeepSeek、Anthropic provider worker；核心不再包含服务商 wire-format 分支。详见[provider 契约与边界](docs/providers.md)。
- 可显式启用 [DeepSeek Files 上传和作用域内 file_id 复用](docs/provider-files.md)；密钥与 HTTP 留在核心，上传失败不自动重试，默认仍用 Base64。[远端管理](docs/provider-files-management.md)支持列表、查询、强制确认删除和显式恢复映射，不调用模型。另支持 [OpenAI Responses/Anthropic Files 与可选生命周期管理](docs/files-lifecycle.md)，自动清理需单独授权。
- 文件读取、写入、精确编辑、Shell、图片；审批、完整工具调用配对、原子会话保存、有界 I/O。
- 可选 Rust 插件：预算、压缩、提示词、记忆、代码搜索、网页、Skill、缓存统计、交互 steering。
- 同一 argv 与规范化 cwd 复用 worker；失败调用**不自动重放**，后续调用才新建进程。
- 参考资料以快照/增量追加，保持已有前缀；缓存命中以**服务端真实 usage**为准。
- 桌面配置集中在 `~/.i/config/`；持久数据、可删除缓存、临时运行文件分开。

核心没有服务商/模型容量表。正式运行时、插件和控制台全部是 Rust；Python 仅用于开发测试与可选示例。

## 安装与启动

从已解压的 bundled 发布包安装，不需要 Cargo：

```sh
./bin/i install       # 默认 ~/.local/bin；--add-to-path 才授权修改 PATH
i setup              # 提供商、URL（官方默认）、模型、Key
i start -C /path/to/project
```

Windows 默认 `%LOCALAPPDATA%\\Programs\\i\\bin`。已有不同版本要求确认或 `--replace`，自动保留备份；PATH 生效前使用输出中的绝对路径。源码用户仍可 `cargo install --path . --locked --features bundled`。详见[安装、凭据和迁移说明](docs/deployment.md)。

完整发布版现在只有一个可执行文件，`i --runtime ...` 提供可选控制台和 worker。仍可用 `cargo build -p i --no-default-features` 构建独立核心，但调用模型需要配置 provider worker 或旁置 `i-plugins`；开发环境保留该独立程序。要求 Rust 1.91+；Linux 构建需 OpenSSL 开发库，运行需系统 OpenSSL/CA 证书。

交互终端直接运行 `i` 会启动控制台；连接配置不完整时先进入向导。向导不调用模型、不猜模型/上下文容量，保留高级设置。缺少已确认容量的新配置明确关闭预算/自动压缩，可用 `--context-window` 启用。`i --runtime init` 仍只创建缺失文件。

新 bundled 配置在启动时从当前可执行文件解析内置 worker，移动安装目录后无需重写默认绑定：

```text
~/.i/config/
  config.json       # 连接设置，引用可移植内置清单
  plugins.json      # 可移植的 profile / disabled 模块选择
  builtin.json      # builtin_profile 描述符，不保存可执行路径
  bindings.json     # 可选的显式自定义 worker 替换
  cache.json        # 默认 observe，不修改服务商缓存选项
```

通常只需运行向导。高级用户采用环境变量凭据时，可手写例如：

```json
{
  "protocol": "openai",
  "base_url": "https://api.deepseek.com",
  "model": "deepseek-flash",
  "api_key_env": "DEEPSEEK_API_KEY",
  "max_tokens": 8192,
  "use_max_tokens": true,
  "plugins": "builtin.json",
  "budget": {"enabled": false},
  "extra_body": {"thinking": {"type": "disabled"}}
}
```

这个例子需要在本机安全设置凭据环境变量。向导另实现了隐藏输入与系统凭据库，只保存随机引用，不把 Key 写进普通 JSON。离线测试未操作个人系统凭据库，实际存取尚未实测。**不要把 Key 写进提示词、记忆、Git、聊天或 CLI 参数**。

```sh
i --runtime console -C /path/to/project --session-name main
```

默认 `standard` 启用通用能力但不启用 Responses 转换；`full` 另包含 Responses。应根据实际服务商选择协议。`use_max_tokens: true` 为 Chat 接口选择 `max_tokens` 字段；否则原生 Chat 使用 `max_completion_tokens`。这属于服务商协议配置，不是旧版本迁移逻辑。

可不启动控制台而直接使用 CLI；调用模型仍需要 provider worker（bundled 已包含）：

```sh
i --config /path/to/model.json -C /path/to/project \
  --session /path/to/session.json '检查项目并提出一个小范围改动'
```

默认自动发现平台配置目录中的 `config.json`，显式 `--config` 优先。CLI 模型、地址、协议、输出预算、插件清单、凭据变量参数覆盖配置；模型/地址也可取自 `API_MODEL` / `API_BASE_URL`。

仓库 `plugins/` 是指向 `target/debug/i-plugins` 的开发示例，新 bundled 安装不依赖它们。旧 `desktop.json` 可明确运行 `i setup --migrate-builtins --confirm` 迁移；发现自定义修改会拒绝，原文件保留。高级用户仍可生成静态绑定：

```sh
i --runtime manifest standard > /tmp/desktop.json
# 检查后替换 ~/.i/config/desktop.json
```

只有旧式/静态清单需要在移动或改变选择后重新生成。新 `builtin.json` 每次启动读取 profile/disabled 和自定义绑定。自定义 worker 的显式路径仍由用户维护；安装/向导不偷偷重写它们。

## 存储与移动端边界

```text
~/.i/
  config/                         # 配置、插件参数、可选提示词文件
  data/
    workspaces.json               # 规范化项目路径摘要 -> UUID；锁与原子写入
    workspaces/<id>/
      sessions/                   # 桌面控制台的命名会话
      memory.json                 # 可选记忆
      metrics/cache-stats.json    # 持久使用统计，不属于可随意删除的缓存
  cache/                          # 可再生成的本地缓存，目前无必需文件
  runtime/                        # 控制台临时队列/IPC
```

四个绝对路径可分别通过 `I_CONFIG_DIR`、`I_DATA_DIR`、`I_CACHE_DIR`、`I_RUNTIME_DIR` 覆盖。worker 请求也可显式携带 `paths` 和 `workspace_id`，无需依赖 HOME。验证会拒绝持久目录放进临时/缓存目录，包括当前可解析的符号链接别名；这不是文件系统沙箱。

```sh
i --runtime paths -C /path/to/project
```

桌面 ID 绑定规范化项目路径。移动项目不是自动状态迁移；移动端显式提供 ID 也不能替代工作区授权。会话文件仍校验原有工作区绑定。

**两个会话入口不要混淆：**控制台把全局会话路径作为 `--session FILE` 交给核心；低级核心独立使用的 `--session-name NAME`、`--sessions list|show|delete` 便利入口仍使用项目内 `.i/sessions`。需要集中存储时使用控制台或显式路径。项目内 `.agents/skills` / `.i/skills` 是资料来源，不是桌面全局状态。

当前实现的是平台路径/请求契约，**没有实现移动端 App、SDK、FFI 或进程内执行后端**。未来 Android/iOS 应提供应用私有目录、工作区访问授权、Keychain/Keystore 凭据，以及静态链接/平台桥接/可选远程执行后端，不能假设能启动任意桌面插件进程。

## 工具与常驻 worker

统一流程：参数验证、能力查找、审批、执行、文本/图片处理、历史配对；`before_tool` / `after_tool` 包围执行。整批调用先验证，再产生副作用；保存点保持调用与结果完整配对。

- `read` / `view_image` 无需审批；写入、编辑、Shell 及**所有外部工具**均需审批。
- `--yolo`（别名 `--dangerously-skip-permissions`）选择自动审批，旧 `--yes` 已移除。仅用于可信环境；显式 ask/deny 仍生效，不隐式授予 Files 清理权限。
- 路径限制在工作区内；读取拒绝非普通文件；图片校验签名、大小与路径。
- 支持 PNG/JPEG/GIF/WebP；每个任务/工具批次最多8张，每张8 MiB；总传输限制可能更早触发。
- Shell 默认60秒，可设 `--timeout`；使用进程组，Windows 使用 `cmd.exe /D /C` 与 Job Objects。
- worker 每次调用15秒，包含阻塞写入；请求/响应帧上限48 MiB。出错即逐出并清理，**不透明重试可能已有副作用的操作**。
- worker 只在当前核心进程内复用，不是跨任务进程的全局 daemon。预算不再依赖内嵌 tokenizer/词表。

复用与故障行为已有测试，**尚未测量性能提升幅度**。JSON、IPC、序列化和网络耗时仍存在。自动生命周期 hook 是可信代码；插件不是沙箱。Shell 继承普通环境但去掉 `I_APPROVAL_TOKEN`；worker 使用环境白名单，不默认继承模型 key 变量。有用户权限的受信任代码仍可能访问文件和其他系统资源。

## 控制台与 Logo

```sh
i --runtime console --bin /path/to/i -C /path/to/project --session-name main
```

- `/steer 文本`：在安全边界应用。
- `/queue 文本`：当前任务结束后提交。
- `/approve` / `/deny`：处理待审批操作。
- `/mode ask|auto`：切换审批模式；`/mode` 查看状态。可替换策略仍能拒绝或询问，详见[审批插件](docs/approval.md)。
- `/exit`：等待当前任务结束后退出；Ctrl-C 中止活动任务并清理控制台子进程。

steering 只有成功保存后才确认；已应用消息持久化，待处理队列不承诺崩溃恢复。stderr 审批帧带每个子进程独立的认证 token。

```sh
i --runtime logo
i --runtime logo --plain --ascii
i --runtime logo --svg > lninl.svg
i --runtime console --no-logo -C /path/to/project
```

`NO_COLOR` 关闭 ANSI。终端与 SVG 共用字形数据，中间的 i 用亮青色突出；纯文本仍保留独立圆点。品牌模块不计入核心1024行，也不写入提示词、工具 schema 或会话。

### 中文交互编辑

控制台已修复中文输入与退格，**未新增第三方库**。按 UTF-8 字符边界插入/删除，支持方向键、Home/End、内存历史和括号粘贴；后台输出不会覆盖草稿，任务完成会显示 `[task finished]`。支持括号粘贴的终端不会因为粘贴内容中的换行而自动提交。输入行使用水平滚动窗口。

这是按 Unicode 标量编辑，不是完整字素编辑器。复杂 emoji、组合符等未覆盖字符会在编辑行显示为 ASCII 转义，提交保留原文。管道/非 TTY 仍按行读取。正常退出、错误及 Ctrl-C 会恢复终端模式；SIGKILL / panic-abort 无法执行清理。详见[控制台边界与测试](docs/console.md)。

## 预算、压缩与参考资料

启用预算插件，并按实际模型显式提供参数，例如：

```json
{
  "budget": {
    "context_window": 1048576,
    "compact_ratio": 0.8,
    "safety_ratio": 0.02,
    "tool_output_ratio": 0.02,
    "image_tokens": 16384,
    "images": "keep"
  }
}
```

这些是配置示例，不是通用模型规格。默认触发阈值为 `min(window × 0.8, window − 输出预留 − 安全量)`。工具输出预算按剩余容量和待执行调用数分配；另有默认1 MiB的 `io_limit_bytes` 资源上限。

预算优先使用服务端返回的输入＋输出 usage，要求锚点匹配模型和历史前缀，再保守估算新增内容并预留工具 schema 开销。首次请求、前缀/模型变化、usage 缺失/不完整/非法时，退回轻量 UTF-8 字节估算；图片需要显式规划值。usage 是请求完成后才知道的，不能预测尚未发送的新输入。本地 BPE tokenizer 与词表已移除，显式请求 BPE 编码会报错，不会悄悄换算法。压缩仍比较一致的内容估算值，schema 不包含可执行清单。估算不是服务端精确计数保证。

压缩已独立为可替换的 **`compact` worker 插件**，不再由预算插件实现。`standard` / `full` 默认启用它；`compact` / `responses-compact` 配置组合包含预算与压缩。`budget` / `responses-budget` 现在只做预算，不自动压缩。默认策略的保留数量、图片处理与提示词文件可放在 `config/compact.json`。

压缩保留完整工具批次及近期历史，写 `.precompact.json` 备份；候选历史必须在可比 token 估算上缩小且满足预算。**摘要请求也在 HTTP 前检查预算**。过大摘要明确失败，目前没有分块摘要。图片可保留、描述或省略，不修改原图。压缩有损并可能重置缓存前缀。

记忆与 Skill 追加不可信快照/增量，不改写 system 或已有历史；内容不变就不追加。压缩丢失基准时，先恢复快照，再进行最终预算检查与提交。

### 替换内置插件

在 `~/.i/config/bindings.json`（或 `$I_CONFIG_DIR/bindings.json`）中设置本机执行绑定：

```json
{"compact":{"exec":["/absolute/path/to/my-compactor"],"cwd":"/absolute/config"}}
```

同样可以替换预算、记忆、缓存等已选用的 worker，所有对应 hook/工具绑定一起替换；不替换核心安全校验或原生工具。压缩插件可以直接返回本地摘要，跳过摘要模型调用。插件失败或结果非法会明确报错，**不会悄悄回退到内置实现**。`init` 保留现有设置，修改后应重新生成并检查桌面清单；源码和发布包启动脚本会自动生成临时清单。详见[替换配置、协议与可运行 Rust 示例](docs/compaction.md)。

## 可选能力

- **记忆**：`memory_get/set/delete`，工作区独立的锁与原子存储；只记用户要求保存的事实/偏好，不存凭据。
- **Skill**：默认 `.agents/skills` / `.i/skills`，只自动发现名称、描述与来源；正文/引用按需读，脚本不自动执行。默认根不允许符号链接逃逸，外部根必须显式配置。
- **代码搜索**：regex/glob/大小写、行号与片段，有遍历上限，跳过隐藏/构建/符号链接/二进制/超大条目。
- **网页**：可配置 GET/POST JSON 搜索；有界公共 HTTP(S) 文本抓取，校验 DNS/地址/跳转，不使用环境代理、不执行 JS；访问私网必须在可信配置中显式放行。
- **提示词**：内嵌默认文本；`config/prompts.json` 选择 system 提示词，默认压缩器使用 `config/compact.json` 的 `prompt` 路径。手工注册 prompts hook 时仍可使用 `--system` / `--compact`。

`compact.json`、`web.json`、`skills.json`、`prompts.json`、`cache.json` 等设置默认从平台配置目录发现；参数和完整协议见 [docs/plugins.md](docs/plugins.md)。DeepSeek 的缓存用量校验、会话恢复前缀测试及诊断说明见 [docs/cache.md](docs/cache.md)。

## 真实缓存指标与99%目标

默认 `observe` 不修改缓存请求选项；明确选择 `openai` / `anthropic` 才添加相应控制，已有字段优先保留。

```sh
i --runtime cache-report -C /path/to/project
```

命中率为 **Σ cache-read 输入 tokens / Σ全部输入 tokens**。OpenAI 已把 cached tokens 包含在总输入里；Anthropic 分母包含普通输入、缓存读取和缓存创建，创建不是命中；DeepSeek 采用实际返回的 hit/miss。usage 不明或准备请求缺少对应响应时，不能认证达标，`target_met` 为 `null`。

统计持久化、加锁、原子更新，不存提示词、工具正文或凭据。`new/append/reset/compact` 是请求序列分类，**append 不等于命中**。冷请求和 miss 都保留；近期记录/stream 数量有界，累计计数保留。

**99%是明确工作负载的可配置目标，不是已达成结果或服务商保证。** 冷启动、新内容、压缩、过期、路由均影响实际缓存。不能填充无用 tokens 或隐藏冷请求来美化指标。

## DeepSeek 低成本实测

先在本机安全设置新的 `DEEPSEEK_TEST_API_KEY`，不要复用已公开的 key：

```sh
cargo build --workspace --locked
work=$(mktemp -d)
./target/debug/i --config "$PWD/examples/deepseek.smoke.json" \
  -C "$work" --max-steps 1 'Reply only OK.'
./target/debug/i-plugins cache-report -C "$work" \
  --config "$PWD/plugins/smoke-cache.json"
```

该配置关闭思考、移除工具、最多生成32 tokens；若服务端仍返回工具调用，会在执行前拒绝，并独立记录 observe 统计。**真实请求仍可能计费**，单次不能验证99%。可用 `--model` 覆盖模型。参数参考：[DeepSeek Chat Completions](https://api-docs.deepseek.com/api/create-chat-completion/)。

thinking + 图片测试可使用 `examples/deepseek.thinking-vision.json`，配合 `-C 图片目录 --image 文件名`。它仍禁用工具，但允许最多1024个输出 tokens。smoke worker 可显式设 `--output-cap 1..1024`，默认保持32。

## 断网重连、代理与会话恢复

API 配置可增加 `"proxy":"socks5h://127.0.0.1:3067"`，通过 Xray SOCKS5 入站并在代理端解析域名；HTTP 入站改用 `http://127.0.0.1:3067`。端口号本身不能判断协议。省略 `proxy` 继承代理环境变量，`"proxy":""` 强制直连。网页工具仍禁用代理、检查实际连接地址，避免绕过私网限制。

`network_retries` 默认5，可设0–8。临时网络错误和可重试 HTTP 状态会按退避/Retry-After 重试，网络阶段总预算180秒；**不重放本地工具、不关闭 TLS 证书校验**。服务端可能已处理丢失响应的请求，重试可能重复计费；smoke 示例明确设为0。缓存统计把缺失响应保留为未知，不伪造命中。

恢复会话：使用同一工作区、同一个控制台 `--session-name`，然后输入“继续”。启动脚本默认使用 `test`；核心也能通过 `--session FILE` 加载检查点。当前没有单独的 `--resume` 参数，不恢复未完成生成或未落盘队列。工具执行中强杀可能留下领先于检查点的副作用，应先核查。详见 [网络配置与恢复语义](docs/network.md)。

## 原生发布包

**下述为共享核心/WASM 重构前的历史测量，不代表当前版本体积。** 当前构建方式、能力边界及继承的网络测试失败见 [WASM 文档](docs/wasm.md)。

原生 release 与 min-size 包生成到 `dist/`。体积随目标平台、编译器、功能和压缩格式变化，请测量实际发布产物，不沿用旧快照数字。`bundled` 包含可选控制台与 worker；参见 [打包说明](docs/packaging.md)。

```sh
# AArch64 额外体积优化；其他架构去掉 --outline。
python3 scripts/package.py --profile min-size --outline
python3 scripts/check_package.py 'dist/*.tar.gz' 'dist/*.tar.xz'
```

打包/检查脚本属于开发工具，发布的 Rust 程序无需 Python。在源码目录设置好 `DEEPSEEK_API_KEY` 后，可用短命令 `./scripts/start.sh [工作区]` 构建并启动，避免长命令粘贴时被换行拆开。

## 行数与验证（历史原生快照）

下表及后续测试数量属于旧原生快照，不代表当前跨平台实现。当前 `src/` 为3165物理行（排除 `src/tests.rs`，包含 packaging/completions 和文件内测试），另计运行时、vendor、JS及资源；旧1024行检查已显式 ignored。当前验证及已知失败见 [WASM 文档](docs/wasm.md)。

| 范围 | Rust物理行数 |
|---|---:|
| 核心，含 HTTP/重试和 worker 传输 | **1024 / 1024** |
| 运行时源码文件，排除独立测试文件但含文件内测试 | **5365** |
| 按上述源码文件口径合计 | **6389** |

六个 provider 文件占运行时1076行，核心加它们已为2100行，尚未包含 Responses 和运行时支持。调用模型必需选用一个 provider，不宣称完整客户端全部必需代码不超过1024行。

globset 已改回未经修改的 crates.io 官方依赖，由 Cargo.lock 锁定，不再维护本地 feature 补丁与源码副本。其许可保留在 THIRD_PARTY_LICENSES.txt。HTTP/重试/worker 传输仍留在核心；服务商编码按新架构移入可替换 provider，单独披露。

包含 rustfmt 后的注释/空行，排除独立测试模块，测试入口声明仍计入。核心测试递归检查 `src/` 生产 Rust 和存在的 `build.rs`，不能把必需核心逻辑藏进插件。`src/tools.json` 是必需静态 schema 资产，单独披露，不算 Rust 行数。

```sh
cargo fmt --all -- --check
cargo build --workspace --locked --features bundled
cargo test --workspace --locked --features bundled
cargo clippy --workspace --locked --all-targets --features bundled -- -D warnings
python3 scripts/check_tls.py --web
python3 scripts/check_console.py
python3 -m unittest discover -s examples -p 'test_plugins.py'
```

当前本地通过 **188个 bundled 工作区 Rust 测试（非 bundled 为182个）、295个 globset 上游测试及5个文档测试、1个 Rust 替换示例测试、3个协议示例测试**，另通过伪终端编辑与本地自签证书拒绝测试，环境为 macOS、mock API。替换网络实现之前的一次真实 DeepSeek `deepseek-flash` 冒烟请求返回 `OK`，退出码0，端到端1.98秒：输入113 tokens、缓存读取0 tokens、无工具、输出上限32 tokens。另一次 thinking + PNG 测试（low effort、上限1024 tokens）在2.13秒内正确识别左侧红色正方形和右侧蓝色圆形：输入349、输出44（含推理33）、缓存读取0 tokens，无工具调用；思考字段非空但未展示。这些单次请求仅验证连通性/基础看图，不验证热工作负载缓存目标或 worker 加速。CI 配置了 Ubuntu/macOS/Windows，但不宣称 Windows 已运行通过、移动端已实现或已有吞吐基准。HTTP 成功/错误体上限48 MiB；文件竞态、硬链接、脱离进程组的子进程、并发会话写入及用户权限代码不属于沙箱保证。
