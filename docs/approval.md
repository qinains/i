# 可替换审批策略与控制台模式

审批策略是可选 Rust 模块 `approval`（`plugins/runtime/src/approval.rs`），不是核心中的文件开关。
核心仍负责最终放行/询问/拒绝、路径约束、执行与检查点；包括 HTTP/worker 传输在内的核心保持在1024物理行以内；选用的 provider 适配代码另计，详见 [providers.md](providers.md)。
这里的审批是**工具执行授权**，不是模型 API 登录或密钥管理；模型凭据不会加入审批事件或 worker 环境。

## 控制台模式

| 命令 | 作用 |
|---|---|
| `/mode` 或 `/mode status` | 显示当前请求模式 |
| `/mode ask` | 默认策略对修改、Shell、外部工具询问；内置 read/view_image/list/glob/search 默认免询问 |
| `/mode auto` | 默认策略自动允许未被策略拒绝的工具，Shell 仍不在沙箱中 |

默认 ask；`--yolo` 与 `--dangerously-skip-permissions` 是同义参数，旧 `--yes` 已移除。控制台只用它们选择初始 auto，不将永久自动审批参数传给核心，之后可用 `/mode ask` 切回 ask。
模式仅在当前控制台及其后续任务内保留，新控制台重新使用默认值/显式参数。
这些控制命令和审批回答不进入输入历史，也不作为任务/steering 发给模型。

控制台在私有运行时临时目录原子写入 `{"mode":"ask"}` / `{"mode":"auto"}`。
每个被选中的 approval worker 会收到 `--mode-file ABSOLUTE_PATH`；内置策略每次重新读取（最多4096字节）。
文件缺失、损坏、未知模式或配置错误导致本次工具调用失败，不能沿用之前的 auto 决定。
控制台会注入 approval 模块，即使使用较旧或不含该模块的清单；有 `module:"approval"` 的入口会复用并应用本机替换。
这与控制台注入 steering 类似，不表示独立核心必须加载此插件。

**模式不等于强制放行。** 替换策略仍能在 auto 下返回 ask 或 deny；控制台不会无条件代答 `y`。
切换只影响后续审批请求：已经显示的询问仍须 `/approve`、`/deny` 或 y/n 回答，不会自动放行，也不能撤销已经开始的操作。
暂不提供“只读规划/沙箱”模式，避免对可信但有完整系统权限的插件作出错误保证。

## 策略协议

使用原有常驻 NDJSON 协议和15秒调用期限，没有新传输层。每次已识别的工具调用，包括内置读取，都会触发 `tool_approval`：

```json
{"id":1,"event":"tool_approval","workspace":"/project","data":{"name":"write","arguments":{"path":"a.txt","content":"hello"},"external":false,"decision":"ask"}}
```

`arguments` 已解析为对象，不同于 `before_tool` 中的 JSON 字符串。输入 decision 是默认决定：
独立核心 `--yolo`（或其长别名）或内置 read/view_image/list/glob/search 为 allow，其余为 ask。只读工具仍经过策略 hook，可被明确拒绝或要求确认。

worker 返回同一 ID，推荐保留输入 data，只修改 decision：

```json
{"id":1,"data":{"name":"write","arguments":{"path":"a.txt","content":"hello"},"external":false,"decision":"deny"}}
```

- `allow`：通过审批，随后仍执行核心路径、参数、I/O 等检查。
- `ask`：要求人工确认；即使独立核心带了 `--yolo`，策略的 ask 仍会询问。
- `deny`：不执行工具。缺失/非字符串/未知 decision 同样不能执行。
- worker 启动失败、错误、超时、错 ID 或非法帧都不回退默认放行，也不自动重试该调用。
- 核心只采用返回的 decision，不用该事件返回的 name/arguments 替换实际调用。
- 拒绝/失败作为工具错误返回，仍保留完整工具批次配对；模型后续显式再发起调用会重新经过审批，不是后台重放。

hook 按清单顺序串联，最终 decision 由核心执行。内置策略保留已有 deny；配置多个自定义策略时，部署者必须审查顺序及组合规则。
注册 hook/替换 worker 是本机信任操作，不是任意代码沙箱。策略可看到工具参数（其中可能含用户提供的敏感数据），不要无意记录或转发。
核心原生的手工会话删除命令仍使用自身询问/`--yolo`，不属于模型工具审批事件。

## 默认策略配置

自动发现 `config_dir/approval.json`，也可显式传 `--config FILE`：

```json
{"mode":"ask","deny_tools":["shell"]}
```

`mode` 可省略（无模式文件时继承核心默认决定）；显式 ask 可收紧独立核心的 `--yolo`。
控制台模式文件优先于配置 mode，但不能绕过 deny_tools 或输入的 deny。
`deny_tools` 为精确工具名数组；未知配置字段、非法类型或模式会报错，避免拼写错误静默削弱策略。
没有配置文件时不自动创建或覆盖用户配置。

## 替换实现

在本机 `bindings.json` 中指定绝对 argv，可选绝对 cwd：

```json
{"approval":{"exec":["/absolute/custom-approval"],"cwd":"/absolute/config"}}
```

控制台会为所选 approval 入口应用此绑定，并追加 `--mode-file ABSOLUTE_PATH`。
替换者需要理解该参数，并明确规定模式如何影响自己的策略；不能仅凭“auto”绕过自己的硬性拒绝规则。
需要人工长时间等待时返回 ask，由核心处理输入，不要阻塞15秒的 worker 请求。

独立运行可用 `i --runtime manifest approval` 生成清单，或手工注册：

```json
{"tools":[],"hooks":[{"events":["tool_approval"],"exec":["/absolute/custom-approval"],"module":"approval"}]}
```

`standard/full` 清单也包含默认 approval 模块。独立核心没有策略 hook 时仍可运行，保留原来的默认审批规则。
生成清单不会改写现有配置；替换配置改变后重新生成独立核心使用的清单。

## 验证范围

离线测试覆盖 allow/ask/deny、`--yolo` 下强制 ask、读取/Shell/外部工具拒绝、非法返回和 worker 失败不回退、绑定替换、模式文件实时更新及损坏失败。
macOS arm64 PTY 检查覆盖 `/mode`、运行中切换及不绕过已待确认请求。没有使用真实模型服务或付费请求；不据此声称 Windows/Linux 终端已执行验证。
