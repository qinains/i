# DeepSeek 缓存与会话前缀稳定性

依据 [DeepSeek 上下文硬盘缓存指南](https://api-docs.deepseek.com/zh-cn/guides/kv_cache)
及 [Chat Completions 用量字段](https://api-docs.deepseek.com/zh-cn/api/create-chat-completion/)。

## 服务商缓存与本地诊断不是一回事

DeepSeek 缓存默认开启，无需添加 OpenAI 的 `prompt_cache_key`、retention 或 Anthropic 的 `cache_control`。
命中需要完整匹配服务商已经落盘的缓存前缀单元。构建有延迟、缓存会淘汰，属于尽力而为，不保证100%命中。
本地文本相同、请求相同或前缀增长都不能证明命中；重复出现在中间也不是相同的开头。
不硬编码旧公告的64-token粒度来预测当前命中数，不以重复付费请求预热缓存。

推荐 `~/.i/config/cache.json`（或 `I_CONFIG_DIR/cache.json`）：

```json
{"mode":"observe","target_hit_ratio":0.99}
```

`observe` 不修改请求体；0.99只是统计目标，不会强制达标或触发重试/预热。
此文件不会补齐模型配置或凭据，也不意味着所有自定义清单自动加载 cache hook。
`standard/full` 已包含 cache；已有个人清单不会被更新程序自动覆盖。

## 历史保存与请求生成

会话仍使用原子 JSON 快照。当前解析/输出使用 serde_json 的稳定对象键排序，数组顺序不变。
消息文本与 Chat Completions 工具 `arguments` 字符串原样保留，不为了稳定哈希而重新解析、排序或排版字符串内部的 JSON。
HTTP 请求结构中的键顺序与服务商的模型 token 序列不是同一概念。

测试覆盖：

- 多次 JSON 快照保存/恢复后，原生 OpenAI、Anthropic 请求构造器输出字节一致。
- standard 插件链（提示词、记忆、Skills、预算、审批、cache 等）及本地 HTTP fixture：
  工具循环、退出进程、改变会话文件排版、恢复后继续请求，旧 messages 前缀字节一致且 tools 不变。
- 带空格的工具参数字符串、中文和工具结果文本不会被会话排版改变。
- 修改文本或请求设置能触发本地差异诊断；统计元数据不发送给模型。

这些是离线回归，不是 DeepSeek 实测命中率。更改提示词、参考资料、工具顺序、模型或压缩历史仍可能改变前缀。
不能为了缓存而丢弃用户修改、冻结过期记忆或关闭必要的压缩。

## Thinking、图片与多轮对话

依据 [思考模式](https://api-docs.deepseek.com/zh-cn/guides/thinking_mode)、
[图像理解](https://api-docs.deepseek.com/zh-cn/guides/vision) 和
[多轮对话](https://api-docs.deepseek.com/zh-cn/guides/multi_round_chat)。以下针对原生 Chat Completions / standard 插件链，不泛化到其他协议适配器。

- thinking 通过 API 配置 `extra_body.thinking` 透传，未显式配置时遵循服务商默认。
- 当前官方规则区分是否携带 tools：带 tools 时历史 reasoning_content 应完整回传，即使某次回答没有调用工具；不带 tools 时回传的该字段会被忽略。
- 核心保存完整 assistant 消息，保留 reasoning_content、content、tool_calls；普通请求只剔除本地 `_` 元数据。
  恢复会话或进入下一轮用户问题时，不自动删除仍保留的 assistant 消息中的推理字段。
- 正常控制台回答只打印 content，不主动展示 reasoning_content；但会话文件确实保存该字段，应按敏感历史保护。
- Chat Completions 无状态，服务商缓存不代替历史回传。
- 原生 `--image` / `view_image` 使用本地 PNG/JPEG/GIF/WebP 的 Base64 data URL，保存在 user 消息内容块中。
  工具图片保留对应 tool 文本结果，图片另附 user 消息；不把图片块直接放进 tool.content。
- 历史图片随会话保存和恢复，不因“上一轮看过”而省略。历史中保存的是当时图片字节，不在恢复时重新读取同名文件。
- 默认仍为内联图片，每次随请求发送；KV 命中不代表省去上传流量。可显式启用 [DeepSeek Files](provider-files.md)：同一作用域复用 file_id，减少上传传输，但不保证降低图片 tokens 或提高 KV 命中。缓存观察转换后的最终模型请求；附加上传不纳入模型 token/cache 统计。换传输方式、凭据或 ID 可能改变前缀。
- 推理与图片影响的输入计算仍按服务商总 usage 统计；没有提供分项字段时，不臆测“图片命中率”或“推理命中率”。
  上一轮新生成的推理属于那次输出，不因属于 thinking 就被计成输入缓存命中；下一次回传时是否计入输入以服务商规则/usage 为准。
- 压缩会替换旧片段及原始前缀。内置图片策略 keep 保留原图，describe 只保留本次摘要提取的视觉描述，omit 不保留被压缩片段的原图。
  仍保留的 assistant 消息继续保留推理字段；被摘要替换的消息不再逐字回传。不能承诺压缩后仍保持原缓存或全部细节。

新增离线回归覆盖显式 thinking enabled、首次图片输入、view_image 工具结果、工具调用及最终回答中的 reasoning_content、
进程重启后的下一轮问题，验证原图和推理内容仍在请求历史中且旧前缀不变；同时验证推理正文、图片 Base64 不进入 cache-report。
该测试验证传输/持久化，不证明真实服务商图片理解能力或缓存命中率。预算中的 image_tokens 仍只是用户配置的估计，不是服务商精确图片计量。

## 用量校验

实际命中率采用返回字段：

```
input_tokens = prompt_tokens
cache_read_tokens = prompt_cache_hit_tokens
uncached_input_tokens = prompt_tokens - prompt_cache_hit_tokens
observed_token_hit_ratio = 累计命中输入 tokens / 累计输入 tokens
```

当 `prompt_cache_miss_tokens` 存在时，必须是非负整数且满足 hit + miss = prompt_tokens。
如果同时返回 `prompt_tokens_details.cached_tokens`，必须与 hit 相等。
负数、非法类型、溢出、数值矛盾均记作未知用量，不当作0，也不继续宣称目标达成。
兼容仅返回命中字段的响应；缺失 miss 不假设其为0。已确认用量的 miss 可由总量减 hit 得到。
冷请求、重试准备记录及未收到响应的请求仍计入统计，不能只挑选命中的轮次。

## 查看前缀诊断

```sh
i --runtime cache-report -C /absolute/project
```

报告的 `recent` 条目增加：

- `request_hash`：cache hook 观察到的请求体 JSON 的 SHA-256，不保存正文。
- `prefix_diagnostic`：
  - `previous_message_prefix_preserved`：先前请求的完整消息数组仍是当前数组的前缀，且其他请求设置相同。
  - `history_changed`：旧消息前缀不再完整保留。
  - `request_settings_changed`：完整 URL 或消息数组之外的请求设置改变。
  - `no_previous_request` / `compaction_request`：没有基线或属于压缩请求。
- `uncached_input_tokens`：该响应确认的未命中输入数；用量无效时为 null。

设置变化是保守诊断，例如输出上限变化不一定改变模型输入前缀，不能等同于缓存失效。
完整请求哈希在正常追加消息时也会变化；比较前缀应看 prefix_diagnostic，而不是要求整包哈希相等。
只存哈希和计数，不把提示词、历史正文、凭据写进缓存统计；哈希不等于匿名化，已知候选内容可能通过猜测比对识别，不应对外随意分享。

standard/full 把 cache 的 before_request 放在请求处理链最后。自定义清单应同样把它放在所有改写请求的 hook 之后；
否则它只能描述自己观察到的中间请求，不能代表之后被其他插件改写的最终请求。核心不会为了诊断新增传输或日志必需逻辑。
已有统计仍保留；此次请求设置指纹算法升级后，同一旧 stream 第一次可能显示一次 reset，不能将它解释为实际缓存未命中。
