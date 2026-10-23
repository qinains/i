# 远端 Files 管理

使用核心 CLI 的 `--provider-action JSON`，与 `i --runtime provider-files` 的**本地映射查看/清理**区分。
这些动作不调用聊天模型、不执行工具、不加载/修改会话，也不进入模型 token/cache 统计。
网络、凭据、同源检查、期限和删除确认仍由核心执行，不把 API key 交给 provider worker。

## 前置配置

沿用 [Files 配置](provider-files.md)：API 配置需要 protocol（deepseek / responses / anthropic；openai 可管理但不上传图片 Files）、base_url、api_key_env、`provider_operations:true`；所选 worker 的 provider.json 需要 account_scope。
GC 报告分类和只读 `files.gc-inspect` 核查入口见 [生命周期恢复文档](files-lifecycle.md#gc-诊断与不明结果核查)；核查不会重置不明删除状态。
管理动作不需要 model，也不要求 `images:files`；可保持 `images:base64`，只开启管理权限。
原有 profile 的 budget 元数据不参与此管理通道。不要覆盖现有配置，按需要合并字段。

以下示例中的 `/path/deepseek.json` 是 API 配置；PROJECT 为对应工作区。

### 列出远端文件（单页）

```sh
i --config /path/deepseek.json -C PROJECT \
  --provider-action '{"op":"files.list","limit":100,"order":"asc"}'
```

DeepSeek 发送 GET /files，固定 purpose=user_data；OpenAI 为 /v1/files + purpose=vision；Anthropic 使用兼容 beta Files 协议，无 purpose/order 参数。limit 为1..1000、缺省100；order 为 asc/desc、缺省asc。
原样返回列表对象，含 data、has_more、last_id；**不会自动遍历所有页**。如有下一页，显式传 after：

```sh
i --config /path/deepseek.json -C PROJECT \
  --provider-action '{"op":"files.list","limit":100,"after":"file-api-..."}'
```

列表是当前远端账号的文件，不仅限当前工作区。输出包含文件名、ID 等敏感元数据，不要随意贴进模型提示或公共日志。

### 查询文件元数据

```sh
i --config /path/deepseek.json -C PROJECT \
  --provider-action '{"op":"files.get","file_id":"file-api-..."}'
```

发送 GET /files/ID，验证 id/object/filename/bytes/created_at/purpose/过期时间类型。
这不是文件内容下载，也不会偷偷改变本地映射。404/403 等直接报错，不把它们擅自当作已删除确认。

### 删除远端文件

先查询并确认目标，再执行：

```sh
i --config /path/deepseek.json -C PROJECT \
  --provider-action '{"op":"files.delete","file_id":"file-api-..."}'
```

核心显示实际 DELETE URL，要求输入 y；**`--yolo` / `--dangerously-skip-permissions` 不跳过普通 files.delete 的确认**（旧 --yes 已移除）。另有独立授权的 [自动清理通道](files-lifecycle.md)，不由这些审批参数开启。
管理动作不能与 task、--image、--session、--session-name、--sessions 混用，也不是控制台里的聊天命令。
实际 DELETE 必须无请求体，同源、不跟随重定向、无自动重试。

当前凭据/工作区内指向该 ID 的本地记录先持久化为 deleting；收到同 ID 的 `object:file, deleted:true` 才改为 deleted。
已确认 deleted 的记录不会自动消失：保留删除标记，防止下一轮模型请求把同一快照偷偷重新上传。
如仍需使用图片，可另建不含旧图的会话，或明确忘记该本地映射，授权以后重新上传。切换凭据/namespace 会生成新的映射，旧作用域的删除标记不等于全局禁止上传该图片。

网络断连、错误响应、进程退出、确认被拒绝或确认等待超时等情况下，记录可能保留 deleting。
它表示已排定但结果未确认，**不证明 DELETE 已发出**。后续模型请求会拒绝使用该记录，而不是冒险使用/重传。
重新执行删除命令是新的显式操作，仍需再次确认；系统不会后台重放旧删除。
未被当前作用域映射跟踪的 ID 也可显式删除，但没有对应本地记录可更新。其他工作区/凭据作用域的记录不被批量改写。

## 恢复 pending/deleting 映射

先用本地命令获得 ENTRY_KEY、内容哈希文件名、大小，再用远端 list/get 核查：

```sh
i --runtime provider-files -C PROJECT
```

如果确认远端文件就是需要复用的图片，可显式恢复该映射，不再上传：

```sh
i --config /path/deepseek.json -C PROJECT \
  --provider-action '{"op":"files.reconcile","key":"ENTRY_KEY","file_id":"file-api-...","confirm":true}'
```

该操作重新 GET 元数据，并检查：

- 本地键属于当前 endpoint/account/credential 作用域。
- 返回 ID 与请求一致；文件名和字节数与本地快照记录匹配。
- 文件未过期或接近过期；不会将原本有过期约束的记录改为永久文件。
- 提交时本地记录仍与请求前快照一致，用于检测可见的并发变化；这不是远端/本地分布式事务，也不提供 ABA 保护。

通过后原子更新为 ready。**文件名中的 SHA256 和大小只是元数据匹配，不是远端内容的密码学证明。** filename 可人为指定，接口没有下载内容校验，因此必须显式 confirm:true；无法确认原始上传来源/内容时不要导入。
跨凭据作用域、没有本地快照元数据的记录、内容下载/逐字节校验、自动找出“最相似”远端文件都未实现。

若确认不再使用映射，仍可用本地 --forget；pending/deleting 需 --ack-unknown，已确认 deleted 仅需 --confirm。
本地 forget 不删除远端文件；远端删除也不删除本地会话的原始图片快照。显式改回 Base64 模式仍会发送保留的历史快照；若不再希望发送图片，需要另建会话或显式移除相应历史，而不只是删除远端 ID。

## 通用 provider 管理契约

核心在管理入口调用 `provider_manage` 而非 `provider_prepare`，输入 action 是用户提供的 JSON 对象。
沿用可选 operation + state / operation_result + operation_state 的循环，最终返回 `{"version":1,"result":...}`，核心仅输出 result。
未知/旧 provider 不支持该事件时明确报错，不回退到模型生成。

附加操作现在允许 POST / GET / DELETE；GET、DELETE 请求体必须为空。普通模型请求仍只能 POST。
普通 DELETE 只在管理入口可用，并在宿主强制人工确认；工具审批插件或 --yolo 不能免去这次确认。额外配置 provider_cleanup:true 后，可信 provider 的 cleanup:true DELETE 才可自动执行；内置 worker 的范围限定见生命周期文档。
其他方法、跨源 URL、非法头、超限请求均拒绝；每次管理仍最多8个附加请求、180秒准备阶段、worker 单次15秒、无自动重试。
确认等待也计入准备阶段期限。方法限制不是对任意 API 语义/恶意本机代码的沙箱，provider 必须可信。

已实现 DeepSeek、OpenAI Responses、Anthropic 兼容 beta Files，以及可选自动刷新/清理。详情与边界见 [files-lifecycle.md](files-lifecycle.md)；不支持跨服务商 file_id 迁移。
没有执行真实服务商 API 操作；验证为本地 HTTP fixtures、状态恢复/竞争测试与已安装程序检查。

官方参考：[list](https://api-docs.deepseek.com/zh-cn/api/list-files/) · [retrieve](https://api-docs.deepseek.com/zh-cn/api/retrieve-file/) · [delete](https://api-docs.deepseek.com/zh-cn/api/delete-file/)
