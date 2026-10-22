# Browser WASM Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans for inline execution, or superpowers:subagent-driven-development if the user selects delegated execution. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 完成浏览器中的真实模型任务、OPFS 文件工具、终端 UI 和 ZIP 结果下载，同时保留原生 CLI。

**Architecture:** Rust library 承载共享工具与回合状态机，Native/WASM 宿主实现异步平台接口。原生保留现有 provider/plugin 策略，浏览器通过小型 JS bridge 使用 OPFS、Fetch、审批及 Blob 下载；不复制 Agent 核心。

**Tech Stack:** Rust 2024 / MSRV 1.91、wasm-bindgen、wasm-pack、异步 OPFS、原生 ES modules、stored ZIP、Node test runner、Playwright Chromium。

**Spec:** `docs/superpowers/specs/2026-10-22-browser-wasm-design.md`（用户已确认）。

## Global Constraints

- 不覆盖其他工作留下的改动；仓库中大量现有源文件尚未跟踪，不能执行 `git add .` 或批量提交整个源码树。
- 浏览器首个模型适配器支持 OpenAI-compatible Chat Completions；原生已有协议支持保持不变。
- 浏览器默认每文件读写上限 8 MiB，工具文本输出上限 1 MiB，每任务最多 32 个模型回合。
- 单次导出限制为未压缩内容总计 64 MiB、10000 个条目；stored ZIP，稳定排序、固定元数据。
- API key 默认仅保存在当前页面内存，不写 OPFS、localStorage、会话、日志或 ZIP。
- 浏览器禁用 shell、进程插件及远端 Files 管理；不静默降级，不模拟成功。
- 已有 Native 路径保护、审批、插件事件、工具批次、检查点与 provider 不透明字段保持行为兼容。
- 不承诺浏览器跨刷新恢复模型历史；OPFS 已完成写入的文件必须持久化。
- 每项提交只包含已审查的目标文件；原本未跟踪的文件首次纳入提交时说明它含既有实现，不冒称全部是本次新增。

## Review Focus

- 中文输入法 Enter 不得提交半成品任务：Task 6 浏览器测试覆盖 composition 状态。
- 批次里最后一个调用损坏时，前面的 write 不得执行：Task 4 用真实副作用计数测试。
- OPFS 配额不足或 writable close 失败时不得显示上传成功：Task 5 注入写入失败测试。
- 导出过程文件被替换或增长不得绕过 64 MiB 限额：Task 3 同时检查 metadata 和实际读取字节。
- 取消期间审批窗口或网络晚到响应不得恢复任务：Tasks 6/7 测试取消后不再产生文件修改。

## 文件职责与接口约定

- `src/lib.rs`：共享模块导出；`cfg` 隔离 Native/WASM。
- `src/platform/{mod,native,wasm}.rs`：异步平台契约与实现，不包含服务商协议。
- `src/workspace/{mod,path,text,archive}.rs`：工作区操作、路径验证、文本语义和 ZIP，不包含 UI。
- `src/agent/{mod,history}.rs`：统一 Agent 循环、工具批次和历史校验。
- `src/browser/{mod,provider}.rs`：wasm-bindgen 导出与浏览器 Chat Completions adapter。
- `web/platform.js`：OPFS、Fetch 和终端回调 bridge；无模型任务循环。
- `web/{app.js,index.html,style.css}`：页面和交互；`web/download.js`：Blob 链接生命周期。
- `tests/{platform,workspace,archive,agent_core}.rs`：共享核心验证；保留现有集成测试。
- `web/tests/`：bridge 单元测试、真实浏览器测试和 mock API fixture。
- `scripts/build_web.sh`、`docs/wasm.md`：可复现构建和使用说明。

所有共享 Rust API 使用 `anyhow::Result`；文件内容用 `Vec<u8>`，路径为经校验的工作区相对路径。异步 trait 不要求全局 `Send + Sync`。下列函数签名是本计划约定，实施不得在邻接任务中另造同义接口。

## Task 1: 建立可编译的双平台 library 与原生基线

**Files:** Modify `Cargo.toml`, `Cargo.lock`, `.gitignore`, `src/main.rs`, `src/platform/native.rs`; Create `src/lib.rs`, `scripts/build_web.sh`; Retire `src/bin/wasm_main.rs`。

**Interfaces:** library 名为 `i`，crate types 为 `rlib, cdylib`；CLI 通过 `i::platform` 访问平台，Web 产物固定为 `web/pkg/i.js`。新增 `native` feature 并纳入 default；原生 bins required-features 包含 native，packaging 隐含 native，浏览器构建使用 `--no-default-features --features wasm`。

- [ ] 保存将修改文件的原始副本到被忽略的 `.i/wasm-baseline/`，列出 git 索引中已有内容；不建立含全部用户文件的初始提交。
- [ ] 运行 `cargo build -p i-plugins` 和 `cargo test --workspace`，记录原始退出码及失败项；现有失败不得归因于本次改造已修好。
- [ ] 加入 library smoke test，先执行 `cargo test --test platform`，确认 library 尚未定义导致失败。
- [ ] 实现 library 和目标依赖隔离：原生 CLI/补全/packaging 依赖仅用于非 wasm32；浏览器依赖受 wasm feature 和 target 双重控制。移除 profile 下误放的依赖项，保留 profile 的实际优化配置。
- [ ] NativeTerminal 不再引用 library 不存在的 `crate::readline`：原生 CLI 将现有 readline 以回调适配，platform 模块只保留不依赖 CLI 的实现。
- [ ] 脚本固定执行 `wasm-pack build --target web --out-dir web/pkg --out-name i --no-default-features --features wasm`；忽略生成产物。Native binary 加 native feature gate，避免 Web 构建误编译原生入口。
- [ ] 验证 `cargo check --workspace`、`cargo check --lib --target wasm32-unknown-unknown --no-default-features --features wasm`、原生基线测试；审查目标依赖树不含 WASM 不支持的进程/线程依赖。
- [ ] 仅提交本任务审查文件，消息 `build: separate native and browser library targets`。

## Task 2: 异步平台契约与共享文件工具

**Files:** Modify `src/platform/{mod,native,wasm}.rs`, `src/main.rs`, `src/tools.json`; Create `src/workspace/{mod,path,text}.rs`, `tests/{platform,workspace}.rs`。

**Interfaces:** `Workspace<F: FileSystem>::new(fs: F, limits: Limits) -> Self`; `async execute(&self, name: &str, args: &Value) -> Result<Value>`。`FileSystem` 提供 async `read_file`, `write_file`, `create_dir_all`, `remove_file`, `read_dir`, `metadata` 和 `canonicalize`，参数为 `&Path`；不存在的 metadata 为 `Result<Option<FileMetadata>>`。`read_dir` 返回 `Vec<PathBuf>`。`Capabilities { files: bool, processes: bool, download: bool }` 决定 `tool_schemas(capabilities) -> Value`。

- [ ] 写失败测试：`reject_workspace_escape` 拒绝 `../x`, `/x`, `C:/x`, `a\\b`, NUL；`native_symlink_escape` 拒绝外部符号链接；`unicode_binary_roundtrip` 精确比较字节；`edit_ambiguity` 保持 line_number 与尾换行语义。
- [ ] 写能力测试：浏览器 schemas 不含 shell；直接 execute shell 返回 unsupported；`list` 可稳定列出嵌套文件；不存在与 permission error 不混淆。
- [ ] 运行 `cargo test --test platform --test workspace` 确认失败，再实现契约、NativeFs 与共享文本工具。将现有 `replace_once`、图片验证和路径保护提取，而非改变其语义。
- [ ] 实现 `Limits::browser()` 的 8 MiB 文件/1 MiB 文本输出边界；原生使用原有配置值，不能悄悄替换现有 CLI 限制。新增 `list` 工具定义及 handler。
- [ ] 原生 `Workspace::tool` 保留原有审批/插件包装，但在批准后调用共享文件执行接口；shell 仍走原有超时/进程组代码。read/write/edit 语义不再两套维护。
- [ ] 运行新增测试及 `cargo test --workspace`，确认原生审批事件顺序、edit 快照和 symlink 保护没有回退。
- [ ] 提交 `refactor: share async workspace file tools across platforms`。

## Task 3: 标准 ZIP 与 download 工具

**Files:** Create `src/workspace/archive.rs`, `tests/archive.rs`; Modify `Cargo.toml`, `Cargo.lock`, `src/workspace/mod.rs`, `src/tools.json`, `src/main.rs`。

**Interfaces:** `DownloadRequest { paths: Option<Vec<String>>, filename: Option<String> }`; `Archive { filename: String, bytes: Vec<u8>, entries: usize }`; `async Workspace::archive(&self, request: &DownloadRequest) -> Result<Archive>`。平台交付与归档分离，download handler 在调用 archive 后交付。

- [ ] 写 `zip_roundtrip`：独立 ZIP reader 断言嵌套 Unicode 名称、空目录、二进制字节、无重复条目，重复导出 bytes 完全一致。
- [ ] 写 `archive_limits_and_paths`：验证 64 MiB 和 10000 条目精确边界、非法 filename/空 paths/不存在路径报错；metadata 与实际字节不一致仍按真实累积量拒绝；输出碰撞不覆盖源文件。
- [ ] 运行 `cargo test --test archive` 确认失败，再使用现有 zip 主版本的 stored writer（关闭不必要默认压缩 features）实现归档；不手写 ZIP 格式。
- [ ] 新增 download schema：可选 paths、filename，默认整个工作区与 `workspace.zip`。Native 默认保存 `.i/downloads/`，默认遍历排除该产物目录；覆盖审批在写输出前执行。显式选择既有产物可以导出，但不得读写同一输出文件。
- [ ] 测试 Native 产物目录排除、覆盖拒绝及空工作区生成合法 ZIP；限额错误不得留下看似完成的输出文件。
- [ ] 验证归档测试、CLI mock tool 调用和 WASM library check，提交 `feat: add bounded workspace ZIP download tool`。

## Task 4: 提取统一 Agent 回合与 Native 宿主

**Files:** Create `src/agent/{mod,history}.rs`, `tests/agent_core.rs`; Modify `src/main.rs`, `src/tests.rs`, `src/lib.rs`。

**Interfaces:** `async agent::run<H: AgentHost>(host: &mut H, history: &mut Vec<Value>, max_steps: usize) -> Result<()>`。`AgentHost` 方法均接收 `&mut self`：async `before_turn(history: &mut Vec<Value>) -> Result<()>` 执行历史 hook 和 compact；async `complete(history: &[Value]) -> Result<Value>` 返回标准 normalized response；async `before_tools(history: &[Value], pending: usize) -> Result<usize>` 返回本批文本输出上限；async `dispatch(call: &Value, image_slots: usize) -> Result<Value>` 返回含 output/可选 images 的工具结果；async `checkpoint(history: &mut Vec<Value>) -> Result<()>` 和 `finish(history: &[Value]) -> Result<()>` 保留宿主策略。同步 `emit(event: Value) -> Result<()>` 输出事件，`cancelled(&self) -> bool` 查询取消状态。

- [ ] 写 FakeHost 测试：真实 write effect counter 在批次末尾调用结构损坏/重复 ID/finish_reason 截断时保持 0；成功多工具结果全部按 ID 配对；保留 reasoning 与 opaque assistant 字段。
- [ ] 写 checkpoint/finish 顺序测试、图片每批最多 8 张测试、工具失败生成 ERROR result 测试、32 回合限制测试和取消后 effect counter 不再增加测试。
- [ ] 运行 `cargo test --test agent_core` 确认失败，再将原生循环逐步提取到 `agent::run`；不得先复制后让两份长期并存。
- [ ] NativeHost 按原顺序桥接 before_turn、compact、tool budget、before_tool、审批/执行、after_tool、checkpoint 与 finish；原生最大步数继续遵循 CLI，而非强制 32。
- [ ] 运行 `cargo test --workspace` 和既有 provider/approval/cache/compaction 集成测试。检查成功/错误工具批次的完整持久化，不把 hook 或 checkpoint 错误吞掉。
- [ ] 提交 `refactor: run native and browser agents through shared core`。

## Task 5: 真实 OPFS bridge 与 WASM 平台

**Files:** Create `web/platform.js`, `web/tests/platform.test.mjs`, `web/tests/opfs.spec.mjs`, `web/package.json`, `web/playwright.config.mjs`; Modify `src/platform/wasm.rs`, `src/lib.rs`。

**Interfaces:** JS 导出 async `opfsRead(path)`, `opfsWrite(path, bytes)`, `opfsMkdir(path)`, `opfsRemove(path)`, `opfsList(path)`, `opfsMetadata(path)`；全部操作锚定私有 workspace handle，metadata 不存在返回 null。Rust `WasmPlatform::open() -> Result<Self>` 与 `OpfsFileSystem` 将 JS rejection 转为有上下文的错误。

- [ ] 写 bridge fake-handle 测试，验证逐段路径校验、不存在/权限错误区分、close 失败 abort、配额错误传播；运行 `node --test web/tests/platform.test.mjs` 确认失败。
- [ ] 实现 async handle API；write 严格等待 close，read 限额在 File.size 与实际 buffer 上双重检查；成功事件不得先于落盘。
- [ ] 创建 Playwright 测试：localhost 实际 OPFS 中文嵌套文件、二进制上传、空目录、删除、刷新后持久化；不使用仅内存 mock 代替本项。
- [ ] 依赖安装限定为 Web 测试所需 Playwright，记录 lockfile；运行 `npm --prefix web test`、`npm --prefix web run test:browser -- --grep OPFS` 和 WASM check。
- [ ] 提交 `feat: persist browser workspace files in OPFS`。

## Task 6: WASM 模型宿主与终端交互

**Files:** Create `src/browser/{mod,provider}.rs`, `web/tests/provider.test.mjs`; Modify `src/lib.rs`, `web/platform.js`, `web/app.js`, `web/index.html`, `web/style.css`。

**Interfaces:** 导出 `BrowserAgent::create() -> Promise<BrowserAgent>`、`configure(config_json: &str)`、`run(task: &str) -> Promise<JsValue>`、`cancel()`、`new_session()`、`upload(path: &str, bytes: &[u8])`、`list_files() -> Promise<JsValue>`、`export_zip(request_json: &str) -> Promise<JsValue>`。导出 ZIP 的返回结构为 `{filename, bytes}`。JS 回调 bridge 提供 `emit(event)`, `approve(request) -> Promise<bool>`, `fetchModel(request) -> Promise<response>`；用显式取消 token 防止晚到结果生效。

`configure`/`new_session` 返回 `Result<(), JsValue>`，`upload` 为 async 并返回 Promise；run/upload/export_zip/configure/new_session 共用运行锁，冲突时返回 busy，不排队隐式执行。`BrowserConfig` 字段固定为 `base_url`, `model`, `api_key`, `protocol`（默认 openai）, `max_tokens`（默认 8192，必须大于 0）, `max_steps`（默认 32，范围 1..32）。`protocol` 只接受 openai，base_url 为不含凭据/query/fragment 的 HTTP(S) URL。

- [ ] 写 provider 编解码测试：endpoint 仅拼接一次 `/chat/completions`、鉴权不进入事件、工具结果 ID/非透明字段保留、非支持协议拒绝、非法响应/HTTP 错误和重定向拒绝。浏览器请求/响应体上限均为 48 MiB。
- [ ] 写 UI 测试：API key 不进入 localStorage/文件/日志，切换目标开启新会话；IME composition Enter 不提交，重复 Enter 不发并行请求；清屏不删除文件/历史。
- [ ] 实现 BrowserHost，调用 Task 4 的 agent::run，不新增模拟响应或第二套循环。Fetch 使用 AbortController，响应读取时累计限额，错误内容截断；不自动重放失败回合。
- [ ] WebTerminal 通过输出事件更新 DOM，所有模型/文件字符串用 textContent；write/edit 异步审批，拒绝反馈 tool result。显示实际步骤和工具状态，非流式回复不得伪装 token stream。
- [ ] 取消时解除等待中的审批、停止后续请求/工具；await 返回后检查 token。运行期间禁用再次运行、上传、切换模型和新会话，不阻塞取消按钮。
- [ ] 实现真实上传/列表刷新、OPFS 不可用时禁用依赖功能、`!command` 拒绝、明确新会话操作。Task 2 执行层拒绝仍需保留。
- [ ] 运行 provider/UI 测试、WASM 构建和原生回归，提交 `feat: connect browser terminal to real model and file tools`。

## Task 7: 浏览器 ZIP 交付与端到端闭环

**Files:** Create `web/download.js`, `web/tests/{download.test,agent.spec,mock-server}.mjs`; Modify `web/app.js`, `web/platform.js`, `src/browser/mod.rs`。

**Interfaces:** `publishArchive({filename, bytes}, container) -> {url, dispose()}`；按钮 export_zip 与模型 download 调用共享 Workspace::archive，模型工具返回「ZIP ready」及页面链接标识，不声称已存盘。

- [ ] 写链接生命周期测试：无即时 revoke、替换链接/页面销毁才 dispose、导出失败不生成成功提示；点击按钮得到 `.zip` 而不是会话 `.txt`。
- [ ] 本地 mock API 顺序返回 list -> write -> read/edit -> download -> final；断言请求含先前 tool result、浏览器 OPFS 内容正确、tool schema 无 shell、实际下载 ZIP 可解压且字节完全一致。
- [ ] 写真实浏览器反例：拒绝 edit 保持原文件；伪造 shell 被拒绝；取消 pending fetch/审批后无新增写入；截断工具批次无副作用；CORS/OPFS 不可用提示真实错误。
- [ ] 实现 Blob/MIME `application/zip` 交付与持久链接；通过运行中操作锁避免上传与导出竞争，且归档自身仍验证实际大小。
- [ ] 执行 `npm --prefix web test`、`npm --prefix web run test:browser`、`scripts/build_web.sh` 和 `cargo test --workspace`；记录浏览器版本与下载文件验证结果。
- [ ] 提交 `feat: deliver workspace ZIPs and verify browser agent end to end`。

## Task 8: 交付文档与最终回归

**Files:** Create `docs/wasm.md`; Modify `README.md`, `README.zh-CN.md`, `docs/size-report.md`（只改受本次影响的声明）。

- [ ] 文档写明 Rust/wasm-pack/Node 测试前提、构建脚本、localhost 静态服务命令、CORS 配置、API key 内存生命周期、取消/审批、下载位置与限额，以及站点数据清除会删除 OPFS 文件。
- [ ] 对照六项目标逐一链接实现文件和通过的测试；重新统计核心代码行数，移除不再成立的体积/行数宣传，不伪造新测量值。
- [ ] 从清空浏览器测试 profile 的基线复跑完整 Web 流程；生成产物后使用普通静态服务器启动，不依赖测试 runner 注入的替身模块。
- [ ] 最终执行 `cargo test --workspace`、`cargo check --lib --target wasm32-unknown-unknown --no-default-features --features wasm`、`scripts/build_web.sh`、Web 单元和浏览器测试，以及仅针对已改 Rust 文件的 rustfmt 检查。
- [ ] 审查 diff、凭据不落盘和未涉及文件未变化；记录测试缺口，不宣称真实付费服务或其他浏览器已经验证。
- [ ] 提交 `docs: document browser WASM build and verified limitations`，将计划复选框与实际结果同步。

## 执行与审查交接

推荐本会话顺序执行：任务接口依赖紧密，且工作区有大量前人未提交文件，单执行者更容易保持变更边界。每个任务先红后绿，验证通过再进入下一项；最终独立审查需要在用户选定执行方式后按对应技能启动。

实施前尚需用户审阅本计划并选择本会话执行或子代理分任务执行。当前只完成设计和计划，尚未安装依赖或修改产品实现。
