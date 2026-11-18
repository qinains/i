# 显式 DeepSeek Files 模式

已实现：核心代发 multipart 上传、file_id 复用、原始图片快照保留、凭据/端点作用域隔离、过期检测，以及失败后不自动重传。
另支持 [OpenAI Responses/Anthropic Files、可选自动刷新与垃圾回收](files-lifecycle.md)。本页以 DeepSeek 默认不刷新/不清理的模式说明。默认仍为 Base64。没有真实付费 API 测试；当前验证使用本地 HTTP fixtures 和官方协议资料。

## 开启：两处明确授权

1. **合并**下列字段到正在使用的 API 配置，不覆盖已有 model、base_url、api_key_env、plugins、budget 等设置：

```json
{"protocol":"deepseek","provider_operations":true}
```

2. 创建或修改所选 provider worker 使用的 `provider.json`（默认 `~/.i/config/provider.json`，也可通过绑定中的 `--config /absolute/provider.json` 指定）：

```json
{
  "images": "files",
  "account_scope": "deepseek-personal",
  "expires_after_seconds": 3600
}
```

account_scope 必须是非空的账号/用途标签，不能填密钥。实际 namespace 还绑定规范化的上传 endpoint 和核心提供的凭据指纹，因此换 API key 不会误用旧 key 的 file_id。换 key、账号标签或端点会创建新的映射，可能重新上传同一图片。

`provider_operations` 授权选中的可信 provider 执行同源、有界的附加请求；普通 DELETE 另需独立管理入口和宿主人工确认；自动清理须额外授予 provider_cleanup:true 并开启 provider 的 auto_gc 策略。它不是 `/mode auto`，也不会改变工具审批。只开一处不能触发内置 Files 上传。
`init` 为新配置创建 `{"images":"base64"}`；不会覆写现有配置或替用户打开上传权限。

- 默认 TTL **3600秒**，不是官方 API 缺省的永久保存。
- 可配置 3600..2592000 秒（1小时..30天）。
- 只有明确设置 `"expires_after_seconds": null` 才省略过期参数，按官方协议永久保存。
- 关闭时设 `images: base64`，并可关闭 provider_operations。已上传文件不会因此远端删除。
- 上传期间改变 provider 配置会报错，不静默换回 Base64。

## 行为

1. 处理本次请求保留的全部历史图片，而不只是新增图片。使用会话中的 Base64 图片快照校验 MIME/签名与大小（PNG/JPEG/GIF/WebP，每张≤8 MiB），不重新读取历史原路径。
2. 按原始字节 SHA256 和端点/账号/凭据作用域查找工作区内映射。主机名大小写、默认端口和 DeepSeek 根路径 /v1 别名作规范化。
3. 缺失时先锁定并持久化 pending 记录，再向核心返回上传计划。
4. 核心验证用户授权、同源、头部、大小、次数和期限，然后发送 **一次** multipart POST。上传不走模型重试策略，也不进入模型 token/cache 命中统计。
5. provider 校验响应 id / expires_at，原子提交 ready 映射；同一请求中的重复图片也复用该 ID。
6. 仅在最终模型请求中把图片替换为 `{"type":"file","file_id":"file-api-..."}`。缓存观察最终请求；会话仍保留原始 Base64 和推理字段，未来可在另一个凭据作用域重新上传。

上传端点由 DeepSeek provider 构造为 `/files`，发送 `purpose=user_data`；TTL 有值时同时发送 expires_after[anchor]=created_at 和 expires_after[seconds]。
实际文件名是内容哈希加扩展名，不包含本机路径或原文件名，便于检查远端未知上传。
显式 image detail 等额外参数目前无法保证在 file 内容块中等价表达，因此拒绝而不是丢弃；这类输入请使用 Base64 模式。

file_id 复用减少重复传输，**不保证省掉图片推理 tokens，也不保证 KV cache 命中**。首次改传输模式或更换 ID 会改变模型请求前缀。
尚未实现磁盘 Blob 外置；完整快照仍进入本地 worker，仍受48 MiB 帧/请求限制，不能靠 Files 模式绕过全部本地容量限制。

## 恢复与显式清理

映射保存在应用数据目录的工作区命名空间：

```
~/.i/data/workspaces/<uuid>/provider-files.json
~/.i/data/workspaces/<uuid>/provider-files.lock
```

记录仅包含 ID、作用域哈希、内容哈希文件名、大小、状态和时间等元数据，不存密钥原文、图片正文或推理文本。指纹和 ID 仍应作为敏感元数据保护。
entries 与 retired 各≤1000条、合计读取≤2 MiB；不会偷偷逐出 pending 或 ready 记录来触发重新上传。

查看当前工作区的本地记录：

```sh
i --runtime provider-files -C /path/to/project
```

遇到过期、被远端删除或确定要重新上传的 ready 记录，先核查远端状态，再忘记对应的**本地映射**：

```sh
i --runtime provider-files -C /path/to/project --forget ENTRY_KEY --confirm
```

如果上传断连、超时、HTTP 错误、响应损坏、进程在上传后提交前退出，记录会留在 pending。
即使重启程序，后续请求也会报 pending/unknown，**不会自动重传、不会自动降级 Base64**。
pending 也可能表示请求尚未真正发出，不能仅凭它判断远端成功或失败。

使用 [远端管理命令](provider-files-management.md) 或官方 Files API 核查内容哈希文件名、大小和时间，并按需要删除远端文件；也可显式 reconcile 恢复映射而不重传。只有你明确接受“再次上传可能产生重复文件”，才能执行：

```sh
i --runtime provider-files -C /path/to/project --forget ENTRY_KEY --confirm --ack-unknown
```

**上述命令不会列出或删除远端文件，只操作本地映射。** 远端 GET/list/DELETE 和显式 reconcile 已提供为独立的 [--provider-action 管理通道](provider-files-management.md)；[自动刷新/垃圾回收](files-lifecycle.md) 可单独启用，默认关闭。
默认已知接近过期（≤60秒）的 ID 直接报错；明确启用 auto_refresh 才会创建新一代文件。服务端提前删除/撤销 ID 的模型请求错误也直接返回，需核查后显式清理，不自动重新上传并重复模型请求。

并发请求使用文件锁；第二个请求看到 pending 会失败而非另发一份。原子文件提交并 fsync 文件，Unix 另 fsync 目录；不宣称抵抗任意文件系统损坏/断电或 exactly-once 远端副作用。手动清空数据目录或更改 namespace 会移除去重/恢复依据。

## 核心通用操作契约

在 provider v1 prepare/parse 上增加可选能力，保持旧 worker 的单请求输出有效。核心输入新增：

- allow_operations：宿主的显式授权。
- allow_cleanup：独立的 provider_cleanup 授权，只有它允许 cleanup:true 的 DELETE 免询问。
- operations_remaining：本次模型调用还能发出的附加请求数量。
- credential_scope：授权附加操作时才提供的 SHA256 凭据指纹，不是密钥原文，不可拿来鉴权。

provider_prepare 可以暂不返回模型 body，而返回：

```json
{
  "version": 1,
  "state": {"opaque": "provider-owned continuation"},
  "operation": {
    "url": "https://api.example/files",
    "content_type": "multipart/form-data; boundary=...",
    "body_base64": "...",
    "headers": {},
    "auth": {"header":"authorization","prefix":"Bearer "}
  }
}
```

上传使用 **POST**；通用附加操作允许 GET / DELETE，但必须无请求体，普通 DELETE 只允许在独立管理入口且强制确认；明确授权的 cleanup:true 操作除外。核心不读取 plan 指定的文件路径，也不接受跨源 URL/重定向。body_base64 是完整网络请求体编码；核心解码并检查大小后发送，不是把文件再次作为 Base64 JSON 上传。
content_type 是单独的描述字段，不允许用 headers 重复覆盖；认证继续由核心注入并标记敏感。

每次模型调用最多8个附加请求，总网络准备阶段期限180秒，单次原始请求体≤16 MiB，响应≤48 MiB，worker 单次15秒、帧≤48 MiB。
成功 JSON 作为 operation_result，原 state 作为 operation_state，连同原始输入重新交给 provider_prepare，直到返回模型请求。
附加请求任何失败都直接终止，无网络重试/自动 provider 重放；普通模型请求仍按原重试策略执行。
状态存储/幂等性策略由所选 provider 管理，宿主的次数和网络限制不能使任意第三方 worker 自动获得 exactly-once 保证。

HTTP、凭据、大小/同源/期限检查仍计入核心；multipart 编码、图片/映射策略在 provider。依赖复用已存在的 sha2/base64 版本，没有另引 HTTP/SDK 栈。

参考：[DeepSeek Files 指南](https://api-docs.deepseek.com/zh-cn/guides/files_api/) · [上传接口](https://api-docs.deepseek.com/zh-cn/api/create-file/)
