# Provider worker：协议与核心解耦

## 当前边界

核心不再包含 OpenAI/Anthropic 的端点拼接、鉴权格式、图片块转换、工具 schema 转换或响应解析分支。
这些实现位于可替换的 `provider` worker：

- `plugins/runtime/src/provider.rs`：选择适配器、构造请求计划、解析响应入口。
- `plugins/runtime/src/provider_wire.rs`：Chat Completions / Anthropic Messages 编码与回放。
- `plugins/runtime/src/provider_files.rs`：显式 DeepSeek Files、上传状态和 file_id 映射；网络仍由核心执行。
- `plugins/runtime/src/provider_manage.rs`：远端列表、查询、删除与显式恢复映射；不调用模型。
- `plugins/runtime/src/provider_file_api.rs` / `provider_lifecycle.rs`：各厂商 Files 格式、可选刷新和受限自动垃圾回收。
- `plugins/runtime/src/responses.rs`：Responses 输入、输出及不透明推理状态回放，也保留旧 hook 入口。

核心仍拥有凭据读取/注入、同源检查、HTTP/TLS/代理/重试和大小限制、worker 隔离环境/期限、工具审批/执行、完整批次和会话检查点。
请求/响应的内部回合结构沿用既有 messages/tool_calls/choices 字段，属于宿主与 provider 的内部协议，不意味着远端必须使用 OpenAI API。

**核心不是不带任何适配器也能调用模型的完整客户端。** bundled 包含官方 provider worker；非 bundled 构建需要显式 provider 绑定，或旁置 `i-plugins`。
服务商适配是可替换依赖，不是删除了这部分功能。HTTP/重试/worker 传输没有移出核心。共享核心与浏览器支持不再以历史的 1024 行为目标；完整源码还包含 provider、安装、向导和可选的系统凭据桥接。选用 OS 凭据模式时 Key 不经过 provider RPC，见 [deployment.md](deployment.md)。

## 选择内置适配器

保留 `--protocol` / API 配置 `protocol` 名称兼容旧配置。它现在是传给 provider 的字符串选择器，核心不维护服务商枚举。
默认字符串仍为 openai，只有兼容性选择含义。

| selector | API | 默认鉴权 | 输出限制 |
|---|---|---|---|
| `openai` | Chat Completions | Authorization: Bearer | max_completion_tokens，可通过 use_max_tokens 切换 |
| `deepseek` | Chat Completions | Authorization: Bearer | max_tokens |
| `anthropic` | Messages | x-api-key；anthropic-version: 2023-06-01 | max_tokens |
| `responses` | OpenAI Responses | Authorization: Bearer | max_output_tokens |

API 地址和模型仍由用户配置，不猜测服务商/模型支持哪些 thinking、effort 或图片能力。例如：

```json
{
  "protocol": "deepseek",
  "base_url": "https://api.deepseek.com",
  "model": "deepseek-flash",
  "api_key_env": "DEEPSEEK_API_KEY",
  "plugins": "builtin.json",
  "extra_body": {"thinking": {"type": "enabled"}}
}
```

上面是提供商选择字段示意，不是完整的预算配置。若 desktop/standard 清单启用了 budget，还必须填写与所选模型匹配的 `budget.context_window` 等预算元数据；不要为绕过配置错误而关闭预算检查。

其他协议可分别选择 `openai`、`responses`、`anthropic`，并填写相应 endpoint、model、api_key_env。
extra_body 的服务商字段校验现在由内置 provider 执行；保留禁止覆盖模型、历史、工具、输出上限、stream、服务端会话状态等控制字段的检查。
自定义 provider 必须自行实现适合其协议的限制；核心不能语义验证任意远端 API 的字段。
模型选项仍使用上述 API 配置/事件输入；`provider.json` 现在配置图片 Base64/Files 策略，默认 Base64。Files 还需要宿主显式授权，见 [provider-files.md](provider-files.md)。

### Responses 兼容注意

新的 `protocol: responses` 已完成转换，不应再注册旧 `responses` hook，否则明确报错，不能双重编码。
推荐搭配 standard 清单；旧 full/responses 清单继续支持原来的 `protocol: openai` + responses hook 用法。
Responses 默认 `store:false` 并显式请求 `reasoning.encrypted_content`，完整保存/回传 output 项，包括 reasoning、encrypted_content、function_call/call_id。
可读 summary 不能替代不透明推理状态；incomplete 响应不能执行工具。

### Thinking 与图片

- DeepSeek：保留 assistant 的 reasoning_content 和工具参数原文；下一轮和会话恢复不擅自删除。支持内联图片。
- Anthropic：system 放顶层；工具结果转成 user/tool_result；原始 assistant content（含 thinking signature、redacted_thinking）完整保存并回放。
- OpenAI Chat：保留消息与工具调用结构，使用 image_url 内容块。
- Responses：转换为 input_text/input_image；函数输出关联 call_id，不混用输出项 id。
- 不把不同服务商的不透明推理字段互相转换。切换协议建议新建会话；没有自动的跨服务商无损迁移保证。
- manual/adaptive thinking 与 effort 的支持范围随模型变化，当前不会凭模型名替用户打开或映射这些选项。

图片入口和会话仍保存 Base64 快照。现已增加**显式授权的 DeepSeek Files 上传、file_id 复用和过期检测**：provider 生成 multipart 计划，核心执行受限同源 POST，上传不重试。默认仍为 Base64，不自动上传。
未知记录需要明确核查与本地清理；远端 list/get/DELETE 和显式 reconcile 见 [管理通道](provider-files-management.md)。[OpenAI Responses/Anthropic Files、自动刷新和受限自动垃圾回收](files-lifecycle.md) 已实现，但生命周期策略默认关闭。完整配置、恢复步骤与多步骤契约见 [provider-files.md](provider-files.md)。

预算 usage 锚点还绑定 endpoint、provider selector/绑定及请求选项的哈希，避免不同提供商使用同名模型时套用旧 token 用量。旧的无作用域锚点在新核心下回退估算，不猜测其所属提供商；作用域原文不写入会话。

## 替换 provider

API 配置可显式指定：

```json
{
  "protocol": "my-service",
  "base_url": "https://service.example",
  "model": "my-model",
  "api_key_env": "MY_MODEL_KEY",
  "provider": {
    "exec": ["/absolute/my-provider", "--config", "/absolute/settings.json"],
    "cwd": "/absolute/config"
  }
}
```

这里 `provider` 是 worker 绑定，不是服务商名字字符串。显式 API 配置优先于清单绑定；cwd 相对 API 配置目录解析。
不把密钥放进 argv、extra_body 或 worker 配置。

也可在 `bindings.json` 中配置 `{"provider":{"exec":["/absolute/my-provider"],"cwd":"/absolute/config"}}`。
`i --runtime manifest` 将它生成为清单顶层 provider 绑定；控制台也应用该绑定。修改后重启控制台，独立核心需重新生成其使用的清单。
未显式绑定时：bundled 启动自身 `--runtime provider`；非 bundled 查找同目录 `i-plugins`。
配置的 worker 不存在、报错、超时、返回错版本等都直接失败，**不回退其他 provider，不自动重放模型请求**。

## NDJSON 合约 v1

沿用现有 worker 的请求 ID、48 MiB 帧上限和15秒期限，不开第二套传输机制。
请求的 event 为 provider_prepare 或 provider_parse，data.version 固定1，data.protocol 是选择器。

prepare 输入包含 base_url、model、history、max_tokens、compact、tools、plugin_tools、use_max_tokens、extra_body。
工具定义不包含本机 exec/cwd；输入没有模型密钥。返回示意：

```json
{
  "id": 1,
  "data": {
    "version": 1,
    "url": "https://service.example/custom-endpoint",
    "body": {"question": "...", "output_limit": 128},
    "headers": {"x-service-version": "1"},
    "auth": {"header": "x-service-key", "prefix": "Key "}
  }
}
```

headers 为字符串值对象；auth 可为 null，或 header/prefix 描述。核心验证最终 URL 与配置 base_url 同源后，才在宿主内附加密钥。
禁止通过 headers 覆盖 Host、Content-Type、Content-Length、Transfer-Encoding、Connection、Proxy-Authorization 等路由/传输头。鉴权头不允许与普通 headers 重复，名称/前缀受 HTTP 类型校验；注入后的凭据头标记为敏感，避免常规 Debug 输出泄漏。
最终模型 HTTP 是 JSON POST，重定向禁止，TLS 验证保持启用；模型请求重试使用同一准备结果，不重新运行 provider_prepare。
显式启用的附加操作可使用完整原始请求体（例如 multipart），最多8次/180秒、每次≤16 MiB，**没有自动网络重试**；成功后通过 operation_result/state 继续 prepare，见 [附加操作合约](provider-files.md)。

管理入口 `--provider-action JSON` 使用独立 provider_manage 事件，复用受限附加操作并最终返回 result；无需 model，不读写会话，不进入普通模型 hook。GET/DELETE 必须空请求体，普通 DELETE 仅管理入口可用，宿主强制确认（--yolo / --dangerously-skip-permissions 不跳过，旧 --yes 已移除）。额外授予 provider_cleanup:true 时才允许可信 worker 的 cleanup:true DELETE 自动执行。

成功模型 HTTP 的原始 JSON 先经过既有 after_response hook（保留缓存统计和旧适配器兼容），再传给 provider_parse；返回：

```json
{"id":2,"data":{"version":1,"response":{"choices":[{"finish_reason":"stop","message":{"role":"assistant","content":"answer"}}]}}}
```

工具调用使用宿主统一 tool_calls 结构，完整批次/ID/参数字符串/结束状态继续由核心验证。原始 usage 仍提供给现有可选缓存/预算策略；自定义服务商若采用不同 usage schema，需要相应策略适配，不能假设当前统计支持未知格式。
provider 是可信代码，不是 OS 沙箱；清空环境、不传入密钥不等于恶意 worker 无法访问当前用户文件。

## 验证与参考

离线测试覆盖三家原有协议、原生 Responses、图片/推理/会话回放、未知服务商的自定义 wire format/鉴权头、凭据不进入 worker 环境/输入、跨源及非法传输头拒绝、worker 失败无兜底。
没有调用付费模型；不据此宣称真实模型理解质量、缓存命中率或其他平台执行已验证。

- [OpenAI reasoning](https://platform.openai.com/docs/guides/reasoning)
- [OpenAI images](https://platform.openai.com/docs/guides/images-vision)
- [Anthropic Messages](https://platform.claude.com/docs/en/api/messages/create)
- [Anthropic thinking/tool workflows](https://platform.claude.com/docs/en/build-with-claude/thinking-tool-workflows)
- [DeepSeek thinking](https://api-docs.deepseek.com/zh-cn/guides/thinking_mode)
- [DeepSeek Files API](https://api-docs.deepseek.com/zh-cn/guides/files_api)
