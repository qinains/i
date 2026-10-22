# 浏览器 WASM 阶段设计

状态：待用户审阅。本文确定设计，不代表功能已实现或验证。

## 1. 目标与边界

用户要求续接已有实现，完成 Platform trait、浏览器 WASM、终端式 Web UI、OPFS 文件系统、禁用 shell 的功能降级，以及结果 ZIP 下载工具。

采用已确认的「共享核心 + Native/WASM 双平台适配」架构。成功标准是浏览器中能够配置模型、提交任务、审批文件修改、执行真实文件工具、刷新后继续看到文件，并下载可解压的工作区 ZIP；原生 CLI 和已有 provider/plugin 能力不得回退。

第一阶段不提供浏览器 shell、进程插件、远端 Files 管理、跨服务商会话迁移或浏览器中的编译执行。浏览器首个模型适配器支持 OpenAI-compatible Chat Completions；其他协议明确显示不支持，不静默降级。原生已有协议支持保持不变。原生 CLI 的全部插件策略不要求在浏览器中重实现。

## 2. 已有实现与问题

- `src/platform/` 已有 trait 与双平台骨架，但文件接口同步；OPFS 多数方法是占位，其中若干操作会假成功。
- `src/main.rs` 的回合、工具、审批和文件操作仍直接依赖原生环境，尚未使用 Platform。
- `src/bin/wasm_main.rs` 只返回占位文本，ZIP 和上传未实现。
- `web/app.js` 返回模拟模型响应，上传未落盘，下载实际是文本文件。
- `Cargo.toml` 未隔离原生依赖，没有 WASM library 入口，并在 profile 下误放了一组依赖配置。
- 仓库尚无提交，且存在其他工作留下的已暂存和未跟踪文件。只提交本次新增设计文档，不纳入其他文件。

## 3. 模块与共享边界

新增 `src/lib.rs`，导出可复用的平台契约、工具语义与回合状态机。Rust library 同时生成 `rlib` 和 `cdylib`；浏览器使用 `wasm-pack --target web` 产物，现有 WASM binary 占位入口由 library 导出替代。

共享核心负责：

1. 工作区相对路径校验、文本 read/write/edit、图片验证、ZIP 归档规则。
2. 工具定义按平台能力过滤、完整工具批次校验、工具结果与调用 ID 关联。
3. 回合状态转换、步数上限、工具错误归入结果、禁止执行未完整返回的工具批次。

原生宿主负责 CLI 参数、现有插件事件/审批策略、provider worker、原生 HTTP、会话检查点及 shell。浏览器宿主负责 DOM、异步审批、Fetch、OPFS 和下载交付。不得为赶进度将浏览器 Agent 循环独立复制一份。

提取采用可独立验证的小步骤；原生现有事件顺序、完整批次保存、图片和协议不透明字段回放行为保持不变。现有核心行数宣传需按提取后的真实统计更新，不能靠移文件继续声称完整实现只有 1024 行。

## 4. Platform 契约

保留文件系统、终端、进程/平台能力几个小接口，而不是把模型协议塞入 Platform。

- 文件读取、写入、目录操作和元数据查询均为异步 `Result`；存在性检查只将不存在视为 false，权限、配额及其他 I/O 错误必须传播。
- 浏览器 JS 对象不强加 `Send + Sync`；不使用 `unsafe` 伪造线程安全。原生特有线程约束留在原生边界。
- 终端提供异步输入/审批与输出事件。Web UI 订阅内容、步骤、工具和错误事件，不解析日志猜测状态。
- 能力至少区分文件访问、进程执行和下载交付。浏览器进程接口始终返回明确的不支持错误。
- 原生文件路径继续解析现存祖先和符号链接，防止逃出工作区；不能用浏览器的词法校验替换原生保护。

## 5. OPFS 与路径规则

使用异步 OPFS，不引入同步句柄、阻塞主线程或 SharedArrayBuffer。通过小型 JS bridge 调用 `navigator.storage.getDirectory()`、逐层 directory/file handle、`getFile()` 和 `createWritable()`，由 Rust 适配接口统一管理错误。

在 origin 私有根下使用独立 workspace 目录。UI 中的 `/workspace` 只是显示路径，工具参数均为工作区相对路径。拒绝绝对路径、`..`、反斜线、NUL 和驱动器前缀；允许正常 Unicode 文件名，空路径仅可用于显式的根目录查询。上传和归档也使用同一校验。

写入需等待 writable close 成功后才报告成功和刷新文件列表；失败时尽可能 abort。列表和递归遍历稳定排序，区分空目录和不存在，元数据返回真实大小。上传使用原始字节，不强制 UTF-8 转换。文本工具对非法 UTF-8 返回错误。

OPFS 不可用时禁用依赖它的任务、上传和导出，并展示原因，不伪装成持久化内存文件系统。说明 HTTPS/localhost 要求、同源隔离、存储配额及清除站点数据会删除工作区。文件修改成功后和页面重新加载时重新读取目录。

## 6. 浏览器模型与终端 UI

保留现有终端式页面布局，补充 endpoint、model、API key 和运行上限配置。API key 默认仅保存在当前页面内存，不写 OPFS、localStorage、会话、日志或 ZIP；非敏感设置可本地保存。切换 endpoint/model 时开始新会话，避免意外向新目标发送旧历史。

浏览器 provider adapter 负责 Chat Completions 编解码；共享核心不包含服务商端点分支。浏览器通过 Fetch 发出真实请求，使用 AbortController 取消，禁止自动重定向；错误展示经过截断且不包含鉴权头。用户配置的服务需要允许页面 origin 的 CORS 请求；本阶段不隐式引入代理服务器。

第一阶段使用非流式模型响应，但逐回合输出真实进展与工具事件，不模拟 token 流。单页面只允许一个活动任务；发送期间禁止重复提交和上传，保留取消按钮。取消后停止后续请求和工具，不能宣称已经成功的文件修改已回滚。

write/edit 在实际执行前通过 UI 明确审批；拒绝结果反馈给模型。上传来自用户主动选择。清屏只清除终端显示，不删除文件或冒充重置模型历史；另设明确的新会话操作。

## 7. 工具与功能降级

浏览器保留 `read`、`write`、`edit` 和 `view_image`。新增只读 `list` 工具，使模型在没有 shell 时仍能发现工作区文件。原生也使用共享文件工具语义，并保留 shell。

shell 同时在三层禁用：不向浏览器模型公布；输入 `!command` 明确拒绝；伪造的 shell/进程工具调用在执行层也拒绝。未知工具和进程插件也不得穿透到 JS 执行或宿主命令。

保留现有 read 的行号/offset/limit、edit 的唯一匹配与 line_number 语义，以及图片类型和大小检查。浏览器默认每文件读写上限 8 MiB，工具文本输出上限 1 MiB，每任务最多 32 个模型回合；上限必须在执行边界检查，不能仅靠 UI。

## 8. download 工具与 ZIP

新增工具接口 `download({paths?: string[], filename?: string})`。省略 paths 表示整个工作区；显式 paths 为非空文件/目录列表，目录递归展开。不存在的路径报错，重复文件去重。filename 默认 `workspace.zip`，只接受无路径的 ZIP 文件名。

Rust 共享归档层生成标准 ZIP，保留相对目录、空目录、Unicode 名称和二进制原始字节。使用确定的排序和固定元数据，拒绝非法归档路径，不产生绝对路径或 `../` 条目。初期采用 stored ZIP，避免依赖原生压缩后端；不承诺压缩率。

单次导出限制为未压缩内容总计 64 MiB、10000 个条目；在读取累积与写归档时检查，超过限制明确失败，不输出截断 ZIP。只包含 workspace 文件，不附加 API 配置或凭据。UI 日志如需导出须另行显式选择，不作为默认 ZIP 内容。

浏览器 UI 下载按钮与模型 download 调用必须走同一导出实现。按钮由用户点击直接交付 Blob 下载；模型调用先生成 ZIP 并展示持久可点击链接，避免把浏览器自动下载拦截误报为文件已经保存。工具成功表示 ZIP 已准备好，不表示用户已保存到磁盘。替换链接或页面销毁时释放 object URL，不能在点击前立即 revoke。

原生 download 将 ZIP 保存到宿主配置的输出目录，默认工作区下 `.i/downloads/`；该自动产物目录不纳入默认递归导出。若输出与源路径重合则拒绝。覆盖已有 ZIP 需要审批，工具结果返回真实路径。浏览器 workspace 中同名普通文件没有特殊隐藏规则。

## 9. 错误与状态一致性

完整校验工具批次的结构和调用 ID 后才产生副作用。单工具失败产生对应错误结果，不遗留未配对 tool_call。拒绝 malformed、截断或超限响应；网络失败不自动重放可能含副作用的整个回合。

写入、上传、ZIP 生成任一步骤失败均不能输出成功提示。文件修改与会话提交分别报告，页面刷新只保证已完成写入的 workspace 文件可见；第一阶段不承诺浏览器跨刷新自动恢复模型历史。

## 10. 验证与交付标准

1. 原生单元/集成测试基线与提取后回归：原有 provider、审批、路径逃逸、工具批次和会话行为保持一致。
2. 共享核心测试：相对路径、Unicode/二进制、edit 歧义、工具能力过滤、恶意 shell 调用、完整批次、步数上限。
3. ZIP 测试：独立 ZIP reader 解压验证内容、空目录、去重、非法路径、导出限额、排除自身产物与覆盖审批。
4. WASM library 使用 `wasm32-unknown-unknown` 构建，生成 Web ES module，页面加载真实产物，不再引用不存在的导出。
5. 真实浏览器在 localhost 验证上传、嵌套文件读写/编辑、刷新持久化、审批拒绝、shell 禁用及 ZIP 下载后解压校验。
6. 本地 mock 模型实现「请求 -> 工具调用 -> OPFS 写入 -> tool result -> 最终回答」闭环；验证取消、错误响应、截断批次与 CORS 失败。
7. 文档提供可复现构建/启动命令、浏览器能力限制和配置说明。没有真实付费模型测试时明确记录，不用 mock 冒充外部模型已验证。

## 11. 参考

- [MDN: StorageManager.getDirectory](https://developer.mozilla.org/en-US/docs/Web/API/StorageManager/getDirectory)：OPFS 入口和安全上下文要求。
- [MDN: Rust to WebAssembly](https://developer.mozilla.org/en-US/docs/WebAssembly/Guides/Rust_to_Wasm)：library 和 cdylib 构建方式。
- [wasm-pack: build](https://wasm-bindgen.github.io/wasm-pack/book/commands/build.html)：Web target 与 ES module 产物。
