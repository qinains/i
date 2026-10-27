# 可选原生连接插件：ChatGPT 与 Claude Console

> **Claude 计费提醒：免填 API key，不等于免费或使用订阅额度。**
> Claude 连接插件使用 Claude Console API OAuth，调用按 Console API 计费，
> **不使用 Claude Pro / Max 套餐额度**。请在授权前确认 Console 的计费与权限。

连接插件是单独安装、单独运行的原生程序，不是核心内置登录功能，也不是工作区的 WASM 工具插件。源码位于 `plugins/connections`；核心只使用通用 `provider.completion: "plugin"` 合约，不依赖 Codex Agent 或 Claude Code。

## 从 Release 安装

1. 打开项目的 [GitHub Releases](https://github.com/qinains/i/releases)，选择已公开版本中的 `i-connections-<target>.zip` 和对应 `.zip.sha256`。连接插件是**独立附件**，不包含在主程序 `i-<version>-<target>-<profile>` 包内；同一个连接包包含 ChatGPT 和 Claude 两个程序。
2. 按操作系统和 CPU 架构选择 target，不把 macOS arm64/x86_64、Linux 和 Windows 的附件混用。以 Release 附件及包内 `BUILD.json` 的 `target` 为准；没有适合本机的附件时，使用下文源码构建，不假定存在通用二进制或包管理器版本。
3. 先对照 `.sha256` 校验 ZIP，再用系统归档工具解压到一个固定、由自己管理的目录，例如 `i-connections/`。摘要只能检查下载完整性，不能代替来源与发布者身份核验。不要把插件放进项目工作区、共享目录或浏览器临时下载缓存后直接长期使用。

以下把下载的实际文件名替换进 `archive`。macOS 可用：

```sh
archive=i-connections-REPLACE_WITH_TARGET.zip
shasum -a 256 -c "$archive.sha256"
```

Linux 可用 `sha256sum -c "$archive.sha256"`。Windows PowerShell 可分别显示实际和预期摘要，再解压到新的目录：

```powershell
$archive = 'i-connections-REPLACE_WITH_TARGET.zip'
Get-FileHash -Algorithm SHA256 -LiteralPath $archive
Get-Content -LiteralPath "$archive.sha256"
Expand-Archive -LiteralPath $archive -DestinationPath .\i-connections
```

解压后是 `bin/i-chatgpt-plugin`、`bin/i-claude-plugin`（Windows 带 `.exe`）、`BUILD.json`、`README.txt`、`LICENSE` 和 `THIRD_PARTY_LICENSES.txt`。保留许可文件；macOS 包未提供 Developer-ID 签名或公证，系统可能要求额外的来源审批。这里不要求关闭系统安全机制。

无需安装 Rust 即可使用匹配本机的 Release 程序。将选用的程序加入 PATH，或使用解压目录中的路径。macOS/Linux 若归档工具未保留执行权限，可先执行：

```sh
chmod u+x ./i-connections/bin/i-chatgpt-plugin ./i-connections/bin/i-claude-plugin
```

Claude 还需按 [官方 ant CLI 安装指南](https://platform.claude.com/docs/en/cli-sdks-libraries/cli/quickstart) 单独安装 `ant`，并确保插件进程的 PATH 能找到它；连接 ZIP 不携带 `ant`。**这条免填 key 的登录仍使用 Console API 计费，不是 Pro/Max 额度。**

## 源码构建与 CLI

```sh
cargo build --locked -p i-chatgpt --release --bins
```

构建得到 `target/release/i-chatgpt-plugin` 和 `target/release/i-claude-plugin`。以下命令假设已将所选程序放入 PATH；也可使用绝对路径。

```sh
i-chatgpt-plugin login
i-chatgpt-plugin status
i-chatgpt-plugin models
i-chatgpt-plugin use <account-id>
i-chatgpt-plugin logout [--account ID]
```

`login` 通过系统浏览器完成官方 OAuth 注册和用户授权。请从 `models` 的当前账户目录中选择模型，再创建独立配置：

```sh
i-chatgpt-plugin configure --model <model-id> --output ./chatgpt.json
i start --config ./chatgpt.json -C .
```

`configure` 写入插件可执行文件的绝对路径，不写 OAuth 凭据，也不覆盖主配置或已有输出文件。不传 `--output` 时输出到插件配置目录；读取命令打印的 `path` 后传给 `i start --config`。模型推理由插件完成，文件操作和审批仍由 `i` 自己的 Agent 工具执行。需要额外工具清单时，按现有部署方式在独立配置中显式设置 `plugins`，不把凭据写进工具清单。

可用全局 `--data-dir /absolute/plugin-data` 为插件指定独立目录；建议使用绝对路径，`configure` 会把规范化后的绝对目录写入 worker 参数，避免核心切换工作目录后使用错误凭据目录。

## 浏览器配对

在**运行浏览器的同一台电脑**启动所选插件，并保持进程运行：

```sh
i-chatgpt-plugin serve --origin https://i.lninl.com --port 0
```

若未添加 PATH，在解压目录的上一级选择运行以下一个命令；不要把它当作浏览器插件 ZIP 上传：

```sh
./i-connections/bin/i-chatgpt-plugin serve --origin https://i.lninl.com --port 0
# Claude Console 改用：
./i-connections/bin/i-claude-plugin serve --origin https://i.lninl.com --port 0
```

Windows PowerShell 对应使用 `.\i-connections\bin\i-chatgpt-plugin.exe` 或 `.\i-connections\bin\i-claude-plugin.exe`，后续参数相同。先确认 `ant` 已安装再选择 Claude；如果只用 ChatGPT，不需要安装它。

1. 打开插件打印的 `/app/#connection-plugin=...` 链接。页面消费配对信息后移除地址栏片段，并新建一个**尚未保存**的连接草稿，不覆盖现有连接。
2. 设置中的连接方式为 **Local connection plugin**。点击 **Check local plugin**，页面读取通用插件清单、账户和实际模型目录，而非猜测服务商。
3. 点击 **Log in to provider**，在系统浏览器完成授权。可取消等待；登录后重新检查账户和模型。
4. 选择账户、模型，检查插件身份，再点击 **Save changes**。未保存的配对不会启用连接。

浏览器配对不要求安装主程序 `i`；只有使用原生 Agent CLI 流程时才另外安装主程序。关闭终端或结束 `serve` 后浏览器不能继续调用该插件；重新启动后请使用新打印的链接重新配对，旧链接不能当成永久下载/登录入口。

顶部「插件」中的 **Local connection plugins** 提供管理入口，但页面不会下载安装原生程序。Claude 使用同样流程，只需将命令换成 `i-claude-plugin serve`。一个助手实例的活动账户由所有连接共享；切换账户会影响其他使用该实例的连接，请重新检查模型权限。删除浏览器连接不等于退出原生账户，退出需单独执行登出。

退出时先持久化清除本地凭据，再执行最长 20 秒的远端撤销或官方 CLI 清理；取消等待不会恢复本地登录。登出结果应区分本地清理与远端撤销授权。结果中的 `remote_revocation_confirmed: false` 表示远端撤销尚未确认，不能解读为“已撤销”；请到服务商设置中检查和移除访问权限。Claude 的官方 CLI 登出完成也不等于已确认服务端撤销。

插件只监听 `127.0.0.1`，使用明确的页面 origin 和随机能力密钥，不提供任意 URL 代理。配对密钥只授权浏览器访问本地插件，**不是 OpenAI/Anthropic OAuth token**；选择记住连接时可单独保存在浏览器，OAuth 凭据仍留在原生侧受保护的配置文件中。不要分享配对链接，或把任何凭据授予工作区工具插件。

凭据文件不作应用层加密，也不宣称存于系统钥匙串。macOS/Linux 使用私有目录和文件权限；Windows 继承所在用户目录的 ACL。请使用自己的私有配置目录，不放进项目、共享目录、版本控制或公开备份；同用户运行的恶意程序仍可能读取它们。

本模式不支持手机独立运行。HTTPS 页面访问 loopback 可能触发本地网络权限提示或被浏览器策略阻止；不能绕过权限、混合内容或 CORS 安全机制。可改用 CLI，或在电脑上以本地 HTTP 静态服务打开 Web UI，并为该确切 origin 重新启动、配对插件。不要把监听地址改成公网地址。

**原生连接插件没有 WASM 沙箱。** 它们是拥有当前操作系统用户权限的进程，应只安装可信二进制。宿主不转发主配置的 API key，并不意味着恶意原生插件无法读取同用户文件；插件私有凭据目录和环境隔离也不是 OS 沙箱。

## ChatGPT 的使用边界

ChatGPT 插件使用自行注册的官方 OAuth 客户端、独立宿主标识及凭据存储，不借用专有客户端 ID，也不读取 `~/.codex/auth.json`。注册、PKCE、授权和权限要求见 [OpenAI 官方登录文档](https://developers.openai.com/siwc/token-sharing-open-source/sign-in)；账户隔离、刷新与退出见 [账户和会话](https://developers.openai.com/siwc/token-sharing-open-source/profiles-and-sessions)。

推理只发往公开的 `https://api.openai.com/v1/responses`，不是 ChatGPT 私有 `backend-api`。模型来自当前账户的实际目录，按服务端可见性和顺序展示；成功必须收到 `response.completed`，中断或失败流不视为成功。见 [模型与推理](https://developers.openai.com/siwc/token-sharing-open-source/models-and-inference)。

套餐授权路径固定 `store: false`、`stream: true`。当前预览不支持 `max_output_tokens`，因此 **ChatGPT 插件忽略界面/配置的最大输出 token 设置**，不将其转换成其他限额字段；不支持的温度、持久会话或托管工具字段也不能按普通 API 路径推断可用。见 [预览限制](https://developers.openai.com/siwc/token-sharing-open-source/preview-limitations)。资格、工作区政策、用户同意和套餐使用额度均由 OpenAI 决定；登录不保证推理权限，不承诺免费 API 计费或无限使用。

## Claude Console 插件

先按 [Anthropic 官方 ant CLI 安装指南](https://platform.claude.com/docs/en/cli-sdks-libraries/cli/quickstart) 安装 `ant`，再运行：

```sh
i-claude-plugin login
i-claude-plugin status
i-claude-plugin models
i-claude-plugin configure --model <model-id> --output ./claude.json
i start --config ./claude.json -C .
```

插件在自己的 `ANTHROPIC_CONFIG_DIR`、隔离 HOME 和 `i` profile 下调用官方 `ant auth login/print-credentials/logout`，不读取已有 Claude Code 或个人 ant profile 凭据。目前支持一个 Console 工作区 profile；不把它描述为多账户管理。

重新登录可能在授权页面切换 Console 组织或工作区，即使本地 profile 名称没有变化。
网页对已有 Claude 注册的再次登录会先要求确认：继续后，后续模型请求可能把当前会话历史发送到新工作区；取消不会启动登录。需要隔离聊天历史时，请新建会话再使用新的工作区。

这是 **Claude Console API OAuth**，不是 Claude Pro/Max 订阅额度；需要相应 Console 组织、工作区、权限和 API 计费资格。OAuth 登录方式见 [官方认证文档](https://platform.claude.com/docs/en/cli-sdks-libraries/cli/authentication)。插件通过 `ant auth print-credentials --access-token` 获取和刷新令牌，以 Bearer 方式调用公开 Messages API，见 [官方脚本认证说明](https://platform.claude.com/docs/en/cli-sdks-libraries/cli/scripting)。无需把 token 粘贴进网页。

## 验证状态

离线测试可以验证请求合约、配置无凭据、配对边界和失败处理，不能证明真实用户已获得服务商权限。完整 OAuth 浏览器授权、实际账户模型目录和收费推理仍需用户同意后现场验证；本次未完成真实 OAuth 登录或付费推理，不将 mock/单元测试当作这些能力的实测。
