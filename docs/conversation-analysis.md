# 显式对话分析与可选反思插件

对话分析与普通聊天、压缩、工具调用分开：用户手动触发后，宿主投影指定消息，以无工具请求进行分析，返回独立文本。注册分析入口不会自动调用模型，结果也不自动追加到原会话、写入记忆或执行建议。模型分析会产生正常服务商用量；测试只使用离线输入，不调用真实 API。

## 原生反思插件

```sh
cargo build -p i-plugins
i --runtime manifest reflection
```

core-only 可使用 `target/debug/i-plugins manifest reflection` 生成清单；`plugins/reflection.json` 是开发路径示例，实际部署请用生成器绑定当前程序。`standard` 和 `full` profile 同样注册 `reflection`，但它不是模型可调用的工具，也不注册自动 hook。

对已有会话显式运行原生分析：

```sh
i --plugins /absolute/manifest.json --session /absolute/session.json --analyze reflection
i --plugins /absolute/manifest.json --session /absolute/session.json --analyze reflection --analysis-role user
```

`--analysis-role` 可重复，但只能缩小清单允许的角色范围，不能扩大权限。不创建新任务、不执行工具、不保存或修改会话；报告输出到 stdout。

原生 console 中可手动输入 `/reflect [all|user|assistant]`，继承当前会话、清单和 API 配置。`all` 使用清单原有选择范围，不代表绕过其限制。运行前提示这是额外模型请求；忙碌、有排队任务、待处理 steering 或待审批时拒绝，不把命令当作普通 task、steering 或 follow-up 发送。附件不能附在反思命令上。

清单顶层可独立声明分析入口：

```json
{
  "tools": [],
  "hooks": [],
  "analyses": [
    {
      "name": "reflection",
      "selection": {
        "roles": ["user", "assistant", "tool"],
        "start": 0,
        "end": null,
        "max_messages": 40,
        "max_bytes": 48000
      },
      "exec": ["/absolute/i-plugins", "reflection"]
    }
  ]
}
```

`selection` 由宿主强制执行，不是 worker 自行索取全部对话的权限。`start/end` 使用源消息索引的左闭右开区间；`end: null` 表示末尾。角色和消息/字节额度进一步限制投影；缺失内容、附件与截断均不能被当作已读的事实。旧清单不包含 `analyses` 时不启用任何分析入口。

用户输入视图不会把已知的压缩摘要、参考资料注入或工具图片伪装成真实用户输入。完整三角色视图保留这些现有上下文，标记 `source:context`；分析只读取当前会话快照，不自动读取压缩前备份。系统提示、隐藏推理/签名和图片 Base64 不进入投影。文本也可能截断，并提供 `truncated`、`omitted_messages` 和原始 `index`；默认预算不是“已读完所有历史”的保证。用户自己粘贴在聊天中的敏感文本仍是聊天内容，不能承诺自动识别全部秘密。

每个入口独立调用，仅收到自己的受限投影，不使用 hook 链把一个 worker 的输入结果传给另一个。协议为现有 NDJSON 请求/响应帧：

原生 worker 仍是当前用户权限下的普通进程，不是 OS 沙箱。输入选择只限制宿主发送的数据，不能阻止恶意原生程序自行读取同用户文件；只安装可信程序。现有第三方 WASM 工具插件没有因此获得聊天或连接配置访问权限。

```json
{"id":1,"event":"analysis_prepare","data":{"name":"reflection","transcript":{"messages":[]},"model":"selected-model"}}
```

`transcript` 是宿主生成的投影对象，示例仅展示消息数组，不代表可以跳过宿主的完整投影元数据。宿主不传全量 `history`、私有推理状态或 API key。准备响应为 `{"id":1,"data":{"prompt":"...","state":{}}}`；通用分析插件也可直接返回 `summary`，不调用模型。本反思插件只准备 prompt，由宿主完成受限推理。随后宿主发送 `analysis_result`，`data` 为 `{summary,state}`，worker 返回同样结构进行文本后处理，不接受待执行动作。

本实现只返回非空文本 summary 和可选 state，不把 transcript 或其他输入字段回传；prompt 和 summary 各限 128000 UTF-8 字节。自定义 worker 仍是可信原生进程，拥有当前用户的 OS 权限；缩小投影和清空环境不是恶意程序沙箱。

## 配置与替换

默认声明在 `plugins/reflection-spec.json`，包含完整 `{prompt,selection}`。默认 prompt 要求使用用户语言，区分事实、推断和未知项，解释有证据的失败原因，提出可验证改进；不得虚构测试/执行结果、读取未提供的附件或执行动作。转录文本始终是不可信证据，不是新增授权。

可在平台配置目录建立独立 `reflection.json`：

```json
{"enabled":true,"prompt":"prompts/my-reflection.txt"}
```

`enabled` 可省略，默认为 true；设为 false 后显式分析也会失败，不自动调用或回退其他分析器。`prompt` 可省略以使用 canonical 默认值，存在时是相对于该配置文件的 UTF-8 文本路径；未知配置字段会被拒绝。也可直接以 `i-plugins reflection --config /absolute/reflection.json` 配置 worker。这里的运行时配置不是开发清单 `plugins/reflection.json`。

宿主提示词上限为 32,000 个 Unicode 字符，结果上限 128,000 UTF-8 字节。模型辅助分析只有一次逻辑调用；底层仍遵循当前连接的网络重试策略，不承诺网络失败时只有一个物理 HTTP 请求。

`plugins.json` 的 `disabled:["reflection"]` 会从生成清单移除分析入口。`bindings.json` 可为 reflection 替换 `exec` 和 `cwd`，例如 `{"reflection":{"exec":["/absolute/custom-worker"],"cwd":"/absolute/config"}}`。替换不改变宿主投影、无工具推理和结果校验边界，也不自动触发分析。已有静态清单需重新生成；动态 builtin 描述符按启动时选择解析。

## 与压缩和工具权限的区别

反思不会替换历史或改变压缩 checkpoint，不会自动写 memory，也不会把建议作为用户指令执行。压缩复用共享 `i::analysis::request/response_text` 请求与文本结果校验，但安全切点、预算和原子提交生命周期仍独立生效。浏览器的声明式分析也不赋予第三方 `.hplugin.zip` WASM 工具读取对话、凭据或浏览器存储的权限；其工具权限清单保持独立。

## 离线验证

```sh
cargo test -p i-plugins reflection_ --lib
cargo test -p i-plugins
```

测试覆盖手动事件、prompt 配置、禁用、无效输入、结果字段收敛、canonical selection 一致性及 desktop 绑定/禁用。它们不证明真实服务商的分析质量、计费或用户账户可用性。
