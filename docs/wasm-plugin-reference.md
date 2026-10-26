# 浏览器 WASM 插件参考

## 默认提示词管理插件

`builtin.prompts` 来自 `plugins/wasm-prompts`，Rust 编译为独立 WASM，通过相同的
Worker、包校验和授权流程运行。持久工作区第一次使用插件时自动安装、启用；用户
禁用或卸载后不会在该工作区自动恢复。顶部「插件」可编辑系统提示词或恢复默认。

模型工具 `manage_prompt` 支持 `action: get|set|reset`，`set` 额外传 `prompt`。
提示词上限 128 KiB UTF-8，保存到插件的工作区隔离 IndexedDB KV，不是连接配置、
项目文件或 API key。AI 的 set/reset 始终走强制确认，即使会话是 auto 模式也不会
自动批准。当前模型调用期间的提示词保持不变，保存结果在下一个用户任务时载入。
插件没有文件、网络或连接配置访问权限；其他插件无法跨 pluginId 读写这些值。

当前浏览器实际只运行一个主 Agent system 提示词，允许完整替换，不是仅追加后缀。
会话标题在本地计算，浏览器没有在运行的自动压缩模型提示词，所以不展示虚假的
“标题/压缩提示词”选项。原生客户端的 system/compact 配置仍由原生 prompts hook
处理，见[原生插件参考](plugins.md)。修改提示词不改变真实工具权限、审批和文件锁。

提示词设置属于本地工作区数据，ZIP 文件备份不包含它；清除网站数据或私有插件
数据会删除设置。编辑器通过工作区锁与原值比较，拒绝覆盖另一个窗口刚保存的版本。

协议来源是 `plugins/wasm-protocol/`、`plugins/wasm-sdk/`、`plugins/wasm-package/`
及 `web/plugins/`。本页描述浏览器工具插件，不描述原生 subprocess/hooks。
入门见 [Rust 插件教程](plugin-development.md)，社区目录提交见[发布指南](plugin-publishing.md)。

## 宿主安全检查与真实权限边界

导入预览、安装、启用以及执行前，宿主都重新检查实际插件包和模块，而不是信任目录、
缓存的报告或插件自称的能力。检查包括包结构与散列、WASM ABI 和模块限制，以及
host policy 允许的清单字段和权限。启用时核对工作区授权不超过包内声明；运行时
按固定版本与散列重新加载校验，并由 broker 对每次 host 请求执行实际授权、参数、
工作区路径与写入审批检查。旧检查报告不会替代当前检查，不能因已安装就绕过启用或运行检查。

WASM 模块禁止所有直接 imports，在 Worker 内执行；插件通过协议请求宿主 broker，
不能直接调用浏览器 API。唯一可授予的资源能力是当前工作区的 `none`、`read` 或
`read-write` 文件访问，以及按 `(workspaceId, pluginId)` 隔离的私有 KV。
私有 KV 是宿主代理能力，不是直接访问浏览器 `localStorage`、IndexedDB 或 OPFS。
应用凭据、连接设置、任意浏览器存储、网络、shell 和进程都不是可申请的插件能力；
API key 不会因启用插件就传入 Worker。

这些检查约束能力，不会扫描并证明代码无害，也不验证发布者身份。恶意插件仍可能
滥用用户已授予的读写能力，或在工具结果中暴露读到的内容。请审核来源和所请求的权限。
如果你主动把 API key 等秘密写入已授权可读的工作区文件、插件参数或该插件的私有 KV，
插件仍可读取这些数据；宿主隔离应用连接凭据并不能保护你另行提供给插件的秘密。

## 包格式

`.hplugin.zip` 根目录只能包含 `manifest.json`、`plugin.wasm`、`LICENSE`。
不包含源代码、依赖目录或额外目录。采用仓库打包器，不手工改 ZIP 或散列。
包最多 10 MiB；清单最多 64 KiB，WASM 最多 8 MiB，LICENSE 最多 256 KiB。
ZIP64、注释和不受支持的条目结构会被拒绝，完整校验由打包器和安装器执行。

```json
{
  "format_version": 1,
  "id": "example.word-count",
  "name": "Word count",
  "version": "1.0.0",
  "runtime": "harness-wasm-v1",
  "entry": "plugin.wasm",
  "sha256": "0000000000000000000000000000000000000000000000000000000000000000",
  "permissions": {"workspace": "read", "storage": false},
  "tools": [{
    "name": "count_words",
    "description": "Count words in a UTF-8 workspace file.",
    "parameters": {
      "type": "object",
      "properties": {"path": {"type": "string", "minLength": 1}},
      "required": ["path"],
      "additionalProperties": false
    }
  }]
}
```

这里的全零 sha256 是**构建前占位符**，打包器写入实际 WASM SHA-256。
散列提供完整性校验，不证明发布者身份，也不是签名或可信认证。

| 字段 | 约束 |
| --- | --- |
| id | 1–64 字节，小写字母开头，后续小写字母、数字、`.`、`-` |
| version | 三段无前导零的非负整数，例如 `1.0.0` |
| runtime / entry | 固定 `harness-wasm-v1` / `plugin.wasm` |
| tools | 1–16 项，本地 name 不重复 |
| tool.name | 1–32 字节，小写字母开头，其后小写字母、数字、`_` |
| tool.parameters | 受支持的 JSON Schema object 子集，不是任意 JSON Schema 实现 |
| permissions.workspace | `none`、`read` 或 `read-write` |
| permissions.storage | 是否允许当前工作区与插件 ID 隔离的私有 KV |

公开模型工具名由宿主按插件 ID 与本地工具名生成。插件内匹配的是本地名称，不要
硬编码宿主生成的公开名称。版本或字节变化后重新打包；同名不同内容不是原地覆盖承诺。

## Rust SDK

插件实现 `Plugin: Default`：

```rust
fn start(&mut self, request: ToolRequest) -> StepOutput;
fn resume(&mut self, response: HostResponse) -> StepOutput;
```

用 `export_plugin!(crate::YourPlugin)` 生成 ABI。调用开始一次 `start`；需要 host 操作
时返回 HostCall，收到对应结果后调用 `resume`。可以多次往返，最后返回 Done 或 Error。
一次 invocation 结束后不能继续使用其状态。

```json
{"type":"start","request":{"id":"invocation-id","event":"tool","workspace_id":"default","data":{"name":"count_words","arguments":{"path":"notes.txt"}}}}
```

```json
{"type":"host_call","request":{"id":1,"op":"fs.read","args":{"path":"notes.txt"}}}
```

```json
{"type":"resume","response":{"id":1,"ok":true,"result":{"text":"hello world"}}}
```

```json
{"type":"done","output":"{\"words\":2}"}
```

错误 host 响应使用 `ok:false,error:string`，不同时带 result。插件输出错误使用
`{"type":"error","error":"description"}`。所有消息 UTF-8 JSON，不是 NDJSON stdin。
tag 的准确序列化形式由协议 crate 的 serde 定义与测试确定。

## Host 操作

| 操作 | SDK helper | args | 成功 result |
| --- | --- | --- | --- |
| fs.read | host_read(id,path) | path | text: UTF-8 string |
| fs.write | host_write(id,path,text) | path, text | bytes: 写入字节数 |
| fs.list | host_list(id,path) | path | entries: path/directory/size 数组 |
| kv.get | host_kv_get(id,key) | key | value: JSON 或不存在值 |
| kv.put | host_kv_put(id,key,value) | key, value | 空 object |
| kv.delete | host_kv_delete(id,key) | key | 空 object |

每次 host ID 从 1 开始严格递增，最多 128 次。args 不能多出或缺少字段。
文件操作约束在当前授权工作区与挂载内，不接受路径逃逸。文件文本严格 UTF-8，
单文件最多 8 MiB，目录列表最多 1000 条。写入受模式审批控制；拒绝会返回错误，
不授权其他路径。插件没有直接 DOM、fetch、shell、WASI 或任意原生文件系统能力。

KV 归属 `(workspaceId, pluginId)`，不同插件和工作区不能通过猜 key 读取彼此存储。
只有清单 `storage:true` 才可调用。不要在插件私有 KV 里保存 API key。

## ABI 与执行限制

WASM 使用 `wasm32-unknown-unknown`，禁止所有 imports。必要导出包括单一 memory：

- `harness_abi_version() -> i32`：版本 1。
- `harness_alloc(len:i32) -> i32`、`harness_free(ptr:i32,len:i32)`。
- `harness_step(ptr:i32,len:i32) -> i64`：高 32 位为响应指针、低 32 位为长度。

优先用 SDK 管理分配和响应，不手写指针约定。模块必须声明最大内存，单个非共享
wasm32 memory 上限 64 MiB；不允许 memory64、线程、GC、异常等被禁用特性。
如果有 table，数量和最大值受校验器约束。模块没有直接 host imports；宿主操作只能通过协议请求 broker。

协议帧最多 16 MiB，最终文本或错误最多 1 MiB。默认 invocation 执行预算 15 秒，
审批等待会暂停该预算。超时或取消终止 Worker，但**已完成文件副作用不回滚**；
不确定结果必须检查文件后再手动重试，宿主不自动重放。

## 安装、启用和更新

构建 ZIP → 顶部「插件」/ 导入 → 检查身份、权限与散列 → 为所选持久工作区显式启用。
安装不等于全局启用。当前任务使用固定的插件版本快照；更新或删除不应在任务中途
替换代码。临时工作区与插件存储支持以实际安装器限制为准，不假装支持临时授权落盘。

常见故障：内存无最大值（使用官方脚本）、非空 imports（误用 WASI）、散列不一致
（重新打包）、host ID 不连续、读写权限不够、不是 UTF-8、超限或未在当前工作区启用。
测试步骤见 [教程](plugin-development.md)，源码行为是最终依据。
