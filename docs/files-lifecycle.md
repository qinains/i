# 多提供商 Files、自动刷新与垃圾回收

默认仍为 Base64；下面能力均需显式配置，不会改写个人设置。只处理既有 PNG/JPEG/GIF/WebP 图片快照（每张≤8 MiB），不是任意文档/PDF 上传器；服务商模型自身的尺寸/像素限制仍适用，本地8 MiB上限不是服务端接收保证。

## 提供商适配

| API protocol | 上传与模型引用 | 服务端过期 |
|---|---|---|
| deepseek | /files，purpose=user_data；type:file + file_id | 默认1小时，可配置1小时..30天，null永久 |
| responses | OpenAI /v1/files，purpose=vision；Responses input_image + file_id | 同上 |
| anthropic | /v1/files，只传 file；image.source={type:file,file_id} | 当前明确使用兼容 beta 协议，必须显式 expires_after_seconds:null |

OpenAI 图片 Files 必须选 `responses`；Chat Completions 的图片接口不支持此引用形状，因此 `openai + images:files` 明确报错，不偷偷改协议/降级 Base64。`openai` selector 仍可进行独立 Files 管理，但本地映射按 selector 隔离，管理已上传映射请沿用 `responses`。

Anthropic 使用 x-api-key、anthropic-version:2023-06-01，以及固定兼容头 anthropic-beta:files-api-2025-04-14，模型请求也携带该 beta 头。不是对无 beta 头的新分页/过期协议的实现。其文件持久保存；本地刷新周期不等于服务端 TTL。

三个适配器均支持 list/get/delete/reconcile。管理入口见 [provider-files-management.md](provider-files-management.md)。Anthropic 的 after 转为 after_id，不支持 order；删除确认是 type:file_deleted，元数据采用 size_bytes/created_at 字符串。OpenAI 列表固定 purpose=vision。管理查询原样输出服务商响应，不把规范化内部字段冒充服务商字段。

API 配置合并示例（保留实际 base_url、api_key_env、model、预算、插件等）：

```json
{"protocol":"responses","provider_operations":true,"provider_cleanup":true}
```

provider.json（DeepSeek/OpenAI）：

```json
{
  "images":"files",
  "account_scope":"my-private-files-pool",
  "expires_after_seconds":3600,
  "auto_refresh":{"enabled":true},
  "auto_gc":{"enabled":true,"grace_seconds":3600}
}
```

Anthropic 的示例：

```json
{
  "images":"files",
  "account_scope":"my-private-anthropic-files-pool",
  "expires_after_seconds":null,
  "auto_refresh":{"enabled":true,"max_age_seconds":86400},
  "auto_gc":{"enabled":true,"grace_seconds":3600}
}
```

`provider_cleanup`、auto_refresh、auto_gc 都默认关闭；可只启用上传/复用，不启用任何刷新/删除。`--yolo` / `--dangerously-skip-permissions` 不会开启上述权限，旧 `--yes` 已移除。

## 自动刷新

- ready 的已知 expires_at 接近过期（≤60秒）时，用会话保留的原始快照创建替代文件，不读原始图片路径。
- 可配置 max_age_seconds（3600..2592000），为永久文件设置本地轮换周期；不延长远端旧文件 TTL。
- 在网络前将新一代映射持久化为 pending，并把旧 ID 放入 retired。成功才更新 ready；断连/错误/崩溃等不明结果仍阻断后续自动上传。
- 手动删除的 deleted 标记不被自动刷新绕过；服务端意外撤销/提前删除造成的模型错误也不触发盲目重传。
- 新 ID 会改变请求前缀，不能承诺缓存命中或节省图片推理 tokens。
- Anthropic 轮换会产生新的永久文件。若没有实际运行清理，旧文件不会因为本地 max_age 自动从云端消失。

## 自动垃圾回收的范围与权限

这是 **images:files 模式的模型请求准备阶段的机会式维护**，也能显式运行；不是后台守护进程。进程不运行/没有请求时不会定时清理，可由用户自行调度管理命令；程序不会安装 launchd/cron 或持久化 API key。切回 Base64 后需要显式 files.gc 或自行调度清理。

只选择：当前作用域、本客户端成功上传并记录 owned:true、已被替代进入 retired、超过 grace_seconds（默认3600，范围3600..2592000）、且当前工作区映射中没有其他活动引用的 ID。reconcile 会拒绝指向清理待定的 ID，并检查 GET 期间可见的 retirement 状态变化。

不会扫描整个远端账号并删除“看起来不用”的文件；不会远端删除当前 ready 映射、导入/reconcile 的 ID、旧版本无 owned 标记的记录、其他凭据作用域记录或 pending/deleting 不明状态。没有完整的全局引用计数：无法发现其他工作区、外部应用或超长在途请求使用同一 ID。**仅在这些 ID 由本客户端独占管理时启用自动清理**；宽限期不是分布式租约保证。

- auto_gc.enabled 与宿主 provider_cleanup:true 必须同时授权，且仍需 provider_operations:true。
- 内置 worker 仅对上述候选发出 cleanup:true 的 DELETE，网络前持久化 deleting + nonce；确认响应匹配后移除 retired 记录。
- 已知服务端过期的 retired 条目额外等待1小时后只清理本地元数据；接近服务端过期时不争抢 DELETE。两个生命周期策略都开启时，也会清理同作用域、确认本客户端上传且服务端过期超过1小时的 ready 本地记录，腾出映射容量；以后再次使用快照会按显式刷新授权重新上传。
- DELETE 断连、错误、响应不匹配等不自动重试，也不在下次维护重放；不明 retired 记录保留供核查，不阻断下一次正常使用新 ID。
- 普通 files.delete 仍强制人工确认，即使设置 yolo 或 provider_cleanup。只有明确标记 cleanup:true 且单独授权的通用操作免询问。
- **宿主清理权限是授予选中可信 worker 的同源 DELETE 能力，不是由宿主证明“文件属于本客户端”的沙箱。** 内置 worker 负责上述来源/引用策略；恶意本机 worker 仍是可信代码边界之外的问题。
- 仍最多8次附加请求、准备阶段180秒，超过本次清理配额的候选留待以后。上传与清理都不计入模型 token/cache 指标。

预览（默认 dry_run，不发送远端请求）：

```sh
i --config API_CONFIG -C PROJECT --provider-action '{"op":"files.gc"}'
```

在显式双重清理授权下执行：

```sh
i --config API_CONFIG -C PROJECT --provider-action '{"op":"files.gc","dry_run":false}'
```

返回本次确认删除数与剩余候选；候选可能正在等待服务端过期安全期，不代表全部马上可删。entries、retired 各最多1000条，合计读取上限2 MiB；不会为腾空间丢弃未知结果。

检查/放弃 retired 的本地跟踪（不会删除远端文件）：

```sh
i --runtime provider-files -C PROJECT
i --runtime provider-files -C PROJECT --forget-retired KEY --confirm
# 对 deleting/unknown 还必须明确接受不明结果风险：
i --runtime provider-files -C PROJECT --forget-retired KEY --confirm --ack-unknown
```

可先 list/get 核查，再用普通 files.delete 显式确认远端删除，然后忘记不明 retired 条目。自动清理不会删除本地会话图片快照，也不保证服务商物理擦除数据。

## GC 诊断与不明结果核查

`files.gc` 继续默认 dry_run。除兼容的 candidates 字段外，报告现在包含当前作用域 retired 的 records、分类 counts、gc_enabled 和 cleanup_authorized；其他作用域的 ID 不会混入报告。

| reason | 含义 |
|---|---|
| delete_remote | 策略条件满足，可在明确清理授权下删除 |
| prune_local | 已经过服务端过期安全期，只需清理本地元数据 |
| wait_expiry | 接近过期或仍在过期安全期，不争抢 DELETE |
| grace_period | 尚未超过替换后的宽限期 |
| referenced | 当前工作区映射仍有活动引用 |
| not_owned | 没有本客户端上传来源证明，不自动删除 |
| unknown_outcome | pending/deleting，需人工核查，不能自动重放 |
| invalid_state / invalid_metadata | 记录异常，先检查本地状态，不据此自动重新上传/删除 |

分类是本地快照，不是远端现状或删除承诺。没有配置清理权限时，delete_remote 也不会被执行。deleted 仍只统计本次确认收到成功 DELETE 响应的数量，不包含本地元数据清理。

对 retired 中的不明记录，使用它的 KEY 发起一次只读核查：

```sh
i --config API_CONFIG -C PROJECT \
  --provider-action '{"op":"files.gc-inspect","key":"RETIRED_KEY"}'
```

此操作只需要 provider_operations，不需要 provider_cleanup：按记录中的 ID 执行同源 GET，验证返回 ID 和元数据类型，并输出 local_status、remote、metadata_matches_snapshot。记录、nonce 和清理状态均不会被重置；并发修改记录/配置时拒绝返回误导性的稳定快照。

**GET 返回存在，不证明先前 DELETE 没有执行或已取消**，旧请求仍可能在服务端处理中。404/403/断连也不会擅自解释成删除成功。文件名/大小相同不是内容的密码学证明。

确认原操作已停止并核查远端状态后，可使用普通 files.delete 进行一次新的、人工确认的删除，再按前述 --forget-retired / --ack-unknown 流程放弃本地跟踪。程序不会因为 inspect 成功就恢复自动删除队列。

## 验证边界

验证使用本地 HTTP fixtures，包括真实 multipart 字节、两家服务的请求/响应形状、重启复用、独立清理权限、刷新后仍使用新 ID、删除丢响应不重放和模型/会话隔离。没有运行真实提供商上传、删除或付费模型调用，因此不宣称已验证实际账号兼容性、延迟、计费或命中率。

官方资料：[OpenAI vision](https://developers.openai.com/api/docs/guides/images-vision) · [OpenAI Files create](https://developers.openai.com/api/reference/resources/files/methods/create) · [Anthropic Files](https://platform.claude.com/docs/en/build-with-claude/files) · [Anthropic beta Files API](https://platform.claude.com/docs/en/api/beta/files)
