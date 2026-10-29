# 使用 i 管理登录插件

> **Claude 使用 Console API 计费，不使用 Pro / Max 套餐额度。**
> 免填 API key 不等于免费，授权前请确认 Console 的组织、工作区与计费权限。

用户操作统一通过 `i`，不用记辅助程序名称或手写连接配置。原生登录插件
拥有操作系统用户权限，与工作区 WASM 工具插件不同，不要混用安装包。

## 终端内完成

启动 `i`，即使尚未配置模型，也可输入本地管理命令：

```text
/plugins
```

选择 ChatGPT 或 Claude，确认权限后安装，再启用插件。输入 `/login`，
选择登录插件，在系统浏览器完成授权，然后选择模型编号或填写有权限的模型 ID。
成功后自动保存连接配置，下一条任务即可使用。也可直接输入：

```text
/plugins install chatgpt
/plugins enable chatgpt
/login chatgpt
```

`/cancel` 取消菜单；任务运行中不能更换插件或连接。
授权和模型目录查询不发送模型生成请求。只有登录确认和模型选择都完成后才切换配置；
失败或取消不覆盖旧连接，切换前会保存私有配置备份。

## 命令行管理

### 切换模型和思考等级

终端中的 `/model` 从当前原生连接插件读取模型目录，可输入编号选择；也可直接指定模型。
普通 API-key 连接不探测模型目录，使用手填模型 ID。不会重新登录或切换账户：

```text
/model
/model status
/model gpt-6.1-sol
/model gpt-6.1-sol high
/model effort high
/model effort auto
```

仅在任务、审批及队列全部空闲时修改。修改按连接配置保存，写入前保留私有备份；
取消目录选择不改变配置。仅切换模型时恢复服务商默认思考等级，避免沿用另一模型的参数。
`auto` / `default` 表示不显式指定等级；其余可填写 `none`、`minimal`、`low`、`medium`、
`high`、`xhigh`、`max`，但并非每个模型支持全部等级，目录也不一定提供能力声明。
设置不发送付费生成请求，实际可用性由服务商在后续请求时校验。
Responses 使用 `reasoning.effort`，Chat Completions 使用 `reasoning_effort`；
Claude 使用 `output_config.effort` 与 adaptive thinking，`none` 请求关闭思考，
只适用于支持相应模式的模型。不要将高等级或关闭思考视为所有模型都支持。
参考：[OpenAI reasoning](https://developers.openai.com/api/docs/guides/reasoning)、
[Claude effort](https://platform.claude.com/docs/en/build-with-claude/effort)。

```sh
i plugins list
i plugins install chatgpt
i plugins enable chatgpt
i login chatgpt
i plugins status chatgpt
i plugins models chatgpt
i plugins logout chatgpt
i plugins disable chatgpt
i plugins update chatgpt
i plugins reconnect chatgpt
i plugins remove chatgpt
```

已有账户可用 `i plugins use chatgpt ACCOUNT_ID` 切换；
`i login chatgpt --account ACCOUNT_ID` 重新授权已有注册，
`i plugins logout chatgpt --account ACCOUNT_ID` 登出指定账户。
重新授权只暂存注册信息，不复制旧的轮换令牌；模型确认前不会改变旧连接的账户。

将 `chatgpt` 换成 `claude` 即可管理 Claude Console 插件。
安装和更新需要确认；更新后重新启用并登录，避免悄悄替换活动连接。
若只更新辅助程序、不更换已有账户，可重新启用后运行 `i plugins reconnect chatgpt`，
保留当前账户目录和模型设置，只将当前连接绑定到校验过的新程序；不会重新登录或发送生成请求。
安装时显示版本查询、下载字节数、摘要校验与保存进度，可用 `/cancel` 取消。
网络连接超时为 10 秒，每次下载最多 120 秒；失败不会自动重试或启用插件。
GitHub 下载使用终端的代理环境变量，不自动继承浏览器代理。需要时可先运行
`HTTPS_PROXY=http://127.0.0.1:3067 i`，地址与端口应替换为自己的代理配置。

也可在 `i` 内通过内置代理管理命令动态切换，默认按用户保存至
`~/.i/config/proxy.json`（设置 `I_CONFIG_DIR` 时跟随该目录），不写入工作区：

```text
/proxy                         # 查看默认和各用途的有效规则
/proxy 3067                    # 等同于 http://127.0.0.1:3067
/proxy :3067
/proxy 127.0.0.1:3067
/proxy socks5://127.0.0.1:3067
/proxy no                      # 强制直连，也支持 noproxy
/proxy env                     # 恢复环境变量/模型原配置
/proxy chatgpt 3067            # 仅 ChatGPT 登录、模型目录及推理
/proxy claude no               # Claude Console 直连
/proxy plugins no              # 插件下载直连
/proxy model 3067              # 普通模型 API 使用代理
/proxy chatgpt default         # 移除 ChatGPT 覆盖，跟随默认
```

规则优先级：用途专属规则、用户默认规则、原环境/配置。显式 `no` 不回退到环境代理。
修改只影响后续操作，不中断或重放正在运行的请求；浏览器及已经运行的独立服务需另行配置。
支持 HTTP、HTTPS、SOCKS5 和 SOCKS5h；本地授权回调不走代理。
不支持在代理地址中填写用户名、密码、查询参数或路径，避免凭据进入命令历史。
本命令是宿主内置功能，不向工作区 WASM 插件开放代理配置或账户凭据。
移除安装选择会保留旧包与凭据，不代表远端撤销。希望登出时，在禁用或移除前
运行 `i plugins logout NAME`；远端撤销未确认时，仍需到服务商设置检查授权。

安装器从官方 GitHub Release 下载匹配平台的连接附件，校验 GitHub 提供的
SHA-256 摘要、ZIP 成员白名单与目标平台，每次调用前再检查程序摘要。
无匹配平台时停止，不执行第三方安装脚本。摘要验证完整性，不代替发布者签名；
macOS 包目前没有 Developer-ID 签名或公证。安装不修改 PATH，不要求管理员权限。

开发者测试自己构建的连接包，可运行 `i plugins install chatgpt --package /absolute/path/package.zip`，
确认后仍检查 ZIP 成员与平台，安装后默认禁用，并在调用前校验程序摘要。
本地包明确标为 `local-build`，不代表经过官方发布验证；只安装自己信任的原生代码。
浏览器显示回调已收到，并不代表登录完成；仍须等待终端完成令牌交换、身份验证和模型选择。

旧版生成的用户级 `desktop.json` 可能仍包含现已进入核心的 `search` / `find_files`。
宿主会在加载时移除可识别的旧内置搜索条目，并将同一用户配置目录中的旧内置 worker
绑定到当前程序，不修改磁盘原文件。自定义 worker 和自定义插件名称冲突仍保留并报错，
不会通过跳过重复校验来绕过注册规则。此类错误发生在模型请求前，无需重新登录。

### 生成失败的诊断

原生 ChatGPT 连接使用 Responses API，发送 `store:false` 和 `stream:true`，不是 Chat Completions。
终端优先解析 SSE；兼容层也可接收 `application/json` 的完整 Responses 对象，
但必须有 `object:response`、有效 ID、`status:completed` 和通过校验的输出。
若响应缺少 `Content-Type`，终端仅根据正文识别 SSE 或 JSON，再执行相同的完整性校验；
明确标注为 HTML 等其他类型的响应仍拒绝。失败诊断只显示格式分类和字节数，不显示正文。
不接受 HTML、Chat Completions 对象、错误 JSON 或尚未完成的工具调用，不因响应格式变化重发请求。
网页本地服务正常收到 SSE 时仍实时转发；缺少类型头或收到 JSON 时，先使用相同校验器
验证完整响应，再向网页提供有效 SSE。该兼容路径会等待响应完成，不自动重发请求。
网页显示的本地 HTTP 400 不一定是上游状态；安全诊断会区分实际 HTTP、网络或格式失败。

生成失败会显示请求准备、授权、HTTP、响应类型或流解析阶段。HTTP 诊断仅保留状态、
已知错误码/参数名和受限 request ID，不输出原始错误正文、提示词或令牌。
网络或响应格式失败不清除现有登录，也不自动切换账户、模型或 API-key 计费路径。

## 配置与凭据

- `<config_dir>/native-plugins.json` 保存安装位置、摘要与启用状态，不含 OAuth token。
- `<data_dir>/connections/accounts/NAME` 保存插件自己的凭据，不交给工作区 WASM 插件。
- 模型配置由登录菜单自动填写，不需要手工生成 `chatgpt.json`。
- macOS/Linux 使用私有目录和文件权限；Windows 继承用户目录 ACL。
- 凭据不作应用层加密，不宣称存于系统钥匙串。同用户恶意原生程序仍可能读取它们。

不要把上述目录放进工作区、共享目录、版本控制或公开备份。
直接填写 API key 的模型连接仍然可用，无需登录插件。

## 浏览器配对

安装、启用后，在运行浏览器的同一台电脑执行：

```sh
i plugins serve chatgpt --origin https://i.lninl.com --port 0
```

Claude 将 `chatgpt` 换成 `claude`。保持终端运行，打开打印的配对链接，
在页面检查插件、账户和模型后保存。配对先创建未保存草稿，不自动替换原连接。
服务重启后使用新链接；链接含本地能力密钥，不要分享。

服务只监听 `127.0.0.1`，绑定指定页面 origin，不提供任意 URL 代理。
HTTPS 页面访问 loopback 可能需要本地网络权限，本功能不绕过浏览器权限或 CORS。
手机不能独立运行原生插件。删除浏览器连接不等于原生账户登出。

## ChatGPT 边界与排错

使用官方 OAuth 动态客户端注册、PKCE、独立宿主标识与凭据存储，
不读取个人 `~/.codex/auth.json`，不借用专有客户端 ID。见
[官方登录文档](https://developers.openai.com/siwc/token-sharing-open-source/sign-in)
与[账户和会话](https://developers.openai.com/siwc/token-sharing-open-source/profiles-and-sessions)。

推理使用公开 Responses API，不是私有 `backend-api`。
资格、工作区政策与授权额度由 OpenAI 决定，登录不保证推理权限。
套餐授权固定 `store: false`、`stream: true`；当前预览忽略最大输出 token 设置，
见[预览限制](https://developers.openai.com/siwc/token-sharing-open-source/preview-limitations)。

失败会区分授权码交换、会话刷新和网络错误，显示 HTTP 状态码，不显示令牌端点
原始响应、凭据或授权码。失败不自动重放授权或模型请求。授权码交换被拒绝时，
重新发起登录，并检查版本、系统时钟、网络及代理；不能仅凭状态码猜测拒绝原因。

## Claude Console 边界

还需按[官方安装指南](https://platform.claude.com/docs/en/cli-sdks-libraries/cli/quickstart)
安装 `ant` 并确保它在 PATH 中。连接包不携带 `ant`，不会自动执行其安装脚本。
之后仍通过 `i login claude` 操作。

插件使用隔离 HOME、`ANTHROPIC_CONFIG_DIR` 与自身 profile，不读取已有 Claude Code
或个人 ant profile。目前支持一个 Console profile；重新授权可能切换工作区，
后续请求可能发送到新工作区，必要时新建会话。**不是 Pro/Max 订阅额度。**

## 验证范围

自动测试使用隔离目录及本地 OAuth/TLS fixture，不使用个人真实 token，
不替用户授权或发送付费请求。离线通过不能证明真实账户的服务商权限。
辅助程序保留内部 worker/服务协议，用户操作统一由 `i` 转发。
