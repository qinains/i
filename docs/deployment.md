# 用户级安装与最小配置

正式发布使用一个 bundled Rust 可执行文件，不需要源码仓库、Cargo、Python 或 `target/`。`scripts/start.sh` 是开发脚本，不是部署入口。以下命令要求 bundled `i`；core-only 仍使用显式外部 worker 和环境变量凭据。

## 安装

解压适合平台/架构的发布包，验证来源和校验值后运行：

```sh
./bin/i install
```

默认安装到 macOS/Linux 的 `~/.local/bin/i`，Windows 的 `%LOCALAPPDATA%\Programs\i\bin\i.exe`。这些是应用选定的用户级约定，不是所有平台唯一的系统目录。支持：

- `--bin-dir /absolute/path`：自定义位置，包括升级既有 `~/.cargo/bin` 安装。
- `--replace`：非交互授权替换；不同的既有文件默认要求确认。自动留下唯一命名的可执行备份。
- `--sha256 HASH`：校验执行文件，不是压缩包。包旁的 `SHA256SUMS` 中存在对应记录时也会校验；未提供独立校验值时会明确提示。
- `--add-to-path`：明确授权修改用户 PATH。POSIX 修改受支持 shell 的登录配置并备份，不覆盖其他内容；不支持的 shell 要手动处理。Windows 修改用户 PATH，不改系统 PATH。现有同名程序优先时会警告，不删除它。

复制写入临时文件、校验、同步后再替换；拒绝目标符号链接和非普通文件。安装不修改 API 配置或会话。PATH 修改失败不回滚已经成功的二进制安装，输出中的绝对路径仍可使用。打开新的登录 shell/会话后再检查 `command -v i`（Windows 可用 `Get-Command i`）；同一 shell 的环境不会被子进程修改。

这是**本地发布包自安装器，不是联网下载器**。尚未实现发布渠道下载、签名认证或自动更新服务；SHA256 防损坏，不证明发布者身份。macOS 包未做 Developer-ID 签名/公证。包管理器安装应使用其自身更新方式。

## 首次配置

```sh
i setup
i start -C /path/to/project
# 交互终端里直接运行 i 也会启动控制台；缺少连接设置时先进入向导。
```

向导只要求连接所需信息：提供商、URL（有官方默认值）、**模型**、Key。模型不能仅从提供商/URL 可靠推断，因此必须指定；不联网列模型、不做付费探测。

| 提供商选择 | 内部协议 | 默认 URL |
|---|---|---|
| `deepseek` | `deepseek` | `https://api.deepseek.com` |
| `openai` / `responses` | `responses` | `https://api.openai.com/v1` |
| `openai-chat` | `openai` | `https://api.openai.com/v1` |
| `anthropic` | `anthropic` | `https://api.anthropic.com` |

自建兼容服务改 URL 即可；不同 wire-format 的自定义 provider 仍需高级配置。模型和 URL 的命令行参数仍可覆盖配置。保留 `~/.i` 和已有 `I_*_DIR` 约定，不静默搬迁用户数据。

无人值守、环境变量方式：

```sh
# 先在安全渠道设置 OPENAI_API_KEY；不要把实际 key 写入命令历史。
i setup --provider openai --model YOUR_MODEL --key-env OPENAI_API_KEY
# 更新既有配置另需 --confirm；保留预算、extra_body 和自定义绑定等高级字段。
```

向导内部生成现有 API schema（`protocol/base_url/model/api_key_env` 等），用户不需要手写这些字段。`provider` 在高级 API JSON 中仍表示 worker 绑定，不是提供商字符串；不要把向导选项直接复制为同名 JSON 字段。

**不猜上下文容量。** 新连接缺少预算时明确写入 `budget.enabled:false`，提示预算/自动压缩未启用。新建配置输出上限为1024 tokens，这是客户端默认值，不是对模型能力的承诺。提供已确认的 `--context-window N` 可启用预算；已有预算和输出设置保留。切换模型时需核实保留下来的高级参数是否适用。传输、I/O、步骤、审批限制不因此关闭。

上传、自动刷新、远端清理、自动批准均不会被向导默认开启；已有用户明确配置的策略不会被覆盖。

## 凭据

- 交互输入先关闭回显，再显示 Key 提示；普通错误、Ctrl-C、SIGTERM 恢复终端模式。SIGKILL/abort 无法执行恢复。
- 可以选择环境变量（提供商默认 `DEEPSEEK_API_KEY` / `OPENAI_API_KEY` / `ANTHROPIC_API_KEY`），也可以选择系统存储。
- macOS：原生 Keychain API；服务名 `org.lninl.i`，每次设置生成独立随机引用。
- Linux：可选 `secret-tool` / Secret Service；缺少或锁定时明确报错，可改用环境变量，不降级为明文文件。
- Windows：PowerShell + 当前用户 DPAPI；加密数据存放在配置目录 `credentials/`，不是跨用户/机器可移植的明文 Key。
- 非交互 `--key-stdin` 从管道读取并保存到系统存储，与 `--key-env` 互斥。不支持 `--key ACTUAL_SECRET`，也不把 Key 作为子进程 argv 或普通 JSON 字段。
- 配置只保存 `I_CREDENTIAL_<random-id>` 引用。宿主直接读取系统存储，环境变量同名非空值优先；Key 不通过 provider worker RPC。系统存储可能要求解锁或重新授权升级后的程序。

配置提交失败会尝试移除新引用；中断/崩溃仍可能留下未引用条目。旧凭据保留，以便配置备份能回滚，不自动清空系统凭据库。凭据仍会在宿主内存中出现；不承诺内存零化或抵抗同用户恶意程序。worker 是可信原生代码，不是沙箱。

**验证边界：** 本地测试没有读取/修改个人系统凭据库，也没有使用真实 API Key。macOS 的原生绑定已编译，隐藏输入/取消已做 PTY 测试，但实际 Keychain 保存/读取尚未实测；Linux/Windows 实现也未作本机执行验证。不能把这些离线测试宣传为跨平台或真实账户认证通过。

## 可移植内置绑定与旧配置

新 bundled 初始化/向导生成 `builtin.json`：

```json
{"builtin_profile":"configured","tools":[]}
```

启动时读取 `plugins.json` 的 profile/disabled 与 `bindings.json` 的显式替换，动态生成当前 `i --runtime MODULE` 的 argv。默认内置插件无需保存绝对可执行路径；移动安装目录后会指向新位置。配置目录仍由 `I_*_DIR` / `~/.i` 决定，不跟着可执行文件一起搬迁。

自定义绑定保持不动，仍由用户负责稳定路径。旧 `desktop.json` 不会被安装器偷偷改写。可明确执行：

```sh
i setup --migrate-builtins --confirm
```

仅迁移与当前 profile/schema、工作目录一致且命令形状可识别的内置清单；可检测的自定义参数、不同 cwd/schema 会拒绝迁移。命令名称/结构不能证明程序来源：若用户自定义程序也命名为 `i` / `i-plugins`，应先审查再授权。迁移备份 API 配置、改变清单引用，保留原 `desktop.json`；单独的 `bindings.json` 和 API `provider` 自定义绑定不被重写。

独立 `i-plugins init` 仍生成供 core-only 使用的显式桌面清单；移动后需重新生成。core-only 对 bundled 专用描述符会报错，不会默默丢掉插件。`i install/setup/start` 是 bundled 保留入口；要把同名词当任务可使用 `i -- setup` 等显式任务形式。

安装器/向导/系统凭据桥接/描述符解析属于额外运行时代码，不包含在1024行核心承诺中。选用系统凭据模式时这些额外代码是必需依赖；完整客户端不是1024行。
