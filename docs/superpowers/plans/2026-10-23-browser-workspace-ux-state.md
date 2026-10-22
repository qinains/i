# Browser Workspace UX and Persistent Sessions Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在用户最新代码上修复已复现回归，实现真正隔离的工作区、独立会话、刷新恢复和终端式 UI。

**Architecture:** OPFS 保存工作区文件，IndexedDB 原子保存会话/工具日志/UI 快照，各 Session 绑定独立 WASM Agent。UI 按工作区/会话/连接拆分；测试请求和正式请求复用浏览器传输边界，不再复制 Fetch 流程。

**Tech Stack:** Rust/wasm-bindgen、原生 ES modules、OPFS、IndexedDB、Web Locks、node:test、Playwright Chromium/WebKit；本轮不新增运行时 npm 依赖。

**Spec:** `docs/superpowers/specs/2026-10-23-browser-workspace-ux-state-design.md`

## Global Constraints

- 基线包含用户未提交的 `src/main.rs`、platform、tests、Web UI/config 变更及未跟踪 `web/i18n.js`；逐文件整合，不覆盖或回滚，不触碰无关 `examples/plugins.json`。
- 默认 direct + Stream；自动恢复不自动发模型请求、不测试连接、不重放工具、不恢复审批授权。
- 文件保留 OPFS 默认 workspace/；新根 harness/workspaces/<UUID>/，显示名不是路径；不同 Session 历史独立。
- 文件请求/响应既有上限 48 MiB、180 秒保留；安全渲染、真实保存状态、凭据明确记忆语义不因 UI 改版弱化。
- 本轮只做 spec 第一阶段；临时工作区、/tmp 挂载、HTML 预览仍属于后续独立计划，不能伪造按钮或宣称完成。
- 使用当前 feature/browser-wasm 工作区直接实现，完成后一次独立审查，不自行 merge/push/publish，不读取真实 key。

## Review Focus

- 截断边界上的 key、带事件属性的模型/文件/会话名称：不泄密、不执行脚本（Tasks 1–2）。
- 新存储版本、损坏 profiles、失效连接 ID：工作区仍可打开，历史不发给其他服务商（Tasks 1、5）。
- 工具完成写文件后尚未保存结果便刷新：明确未知，不回滚文件、不重放（Task 5）。
- OPFS 目录创建成功但 IndexedDB 事务失败：不假成功，重开可恢复孤立目录（Tasks 3–4）。
- 两个窗口同时恢复同一 Session 或切换 Tab 时流仍返回：不覆盖草稿、不串审批/消息（Tasks 4、6）。

## Task 1: 固化已复现配置/安全问题并恢复 PWA 基础

**Files:** Modify `web/config.js`, `web/app.js`, `web/index.html`, `scripts/build_pwa.mjs`, `web/tests/config.test.mjs`, `web/tests/agent.spec.mjs`, `web/tests/pwa.spec.mjs`; create `web/tests/security.spec.mjs`, `web/connections.js`, `web/render.js`; incorporate existing `web/i18n.js`。

**Interfaces:** `renderText(container,text)` / `renderMessage(record) -> HTMLElement` 不接收可信 HTML；`normalizeConfiguration(input)` 补旧字段；连接记录 `{id,name,settings}`，`settings` 中凭据仅显式记忆时持久化；保留旧 config 函数兼容迁移调用。

- [ ] 写 RED：旧 v1/v2 缺 custom_headers 可加载；损坏配置不阻塞 Ready；模型/目录/会话 payload 不执行 HTML；清所有凭据后 reload 不恢复 key；关闭记忆刷新也不恢复；存储拒绝不显示已保存。
- [ ] 跑 `npm --prefix web test` 与上述 Chromium 用例，记录旧有 4 项配置失败及新失败，不能仅给测试 fixture 补字段掩盖升级回归。
- [ ] 实现旧字段默认与逐条 profiles 迁移、稳定连接 ID、显式凭据清理；断开配置和 Session 历史清理，配置错误隔离。先移除 CDN Prism，正文/代码安全文本显示，保留消息复制，不增加 Markdown HTML 渲染依赖。
- [ ] 恢复可用的 PWA 安装/更新/状态控件（暂放设置中），i18n.js 纳入构建清单；禁止 `.catch(()=>{})` 吞掉初始化错误。新 UI 未完成前保持现有页面可用。
- [ ] 构建 `sh scripts/build_web.sh`；Node 测试和 security/config/PWA 定向测试 GREEN；提交本任务路径，附明确基线范围。

## Task 2: 统一连接测试和正式请求，保留取消

**Files:** Create `web/transport.js`, `web/tests/transport.test.mjs`; modify `web/platform.js`, `web/app.js`, `src/browser/provider.rs`, `src/browser/runtime.rs`, `web/tests/platform.test.mjs`, `tests/browser_provider.rs`, `web/tests/mock-server.mjs`。

**Interfaces:** `normalizeEndpoint(baseUrl) -> string`、`validateCustomHeaders(json) -> Record<string,string>`、`requestCompletion({endpoint,body,apiKey,transport,relayToken,customHeaders,controller,onEvent}) -> Promise<string>`；`fetchModel` 兼容现有 Rust 导入并委托统一请求。BrowserConfig 新增默认空 custom_headers，真实请求传同名选项。

- [ ] 写 RED：Base URL 与完整 endpoint 规范化一致，测试/真实请求头一致；非法 JSON/非字符串值/保留头（Authorization、Host、Cookie、Origin、Content-Type、Accept、X-Harness-*）明确拒绝；relay 不转发任意自定义头并在 UI 提示限制。
- [ ] 增加 RED：连接测试使用选定 token 字段、限额/timeout、可取消、不在 reload 自动触发；错误正文先脱敏再裁剪。测试输入只用本地 fixture 和 dummy key。
- [ ] 运行 Node/浏览器定向测试观察 RED；实现传输接口，测试连接调用相同路径，使用真实 HTTP 状态；测试请求文案注明可能计费，不接受错误 JSON 为成功。
- [ ] 恢复发送/停止按钮和忙碌状态锁；任务期间不改连接、保存新 key 或清凭据，取消贯穿 reader/relay/Agent。
- [ ] 跑 Node、Rust provider、真实 SSE 闸门/relay/cancel 用例 GREEN；提交传输任务文件。

## Task 3: 修复 Native 回归并建立真实工作区根

**Files:** Modify `src/main.rs`, `src/platform/{mod,native,wasm}.rs`, `src/tests.rs`, `tests/archive.rs`, `tests/platform.rs`, `web/platform.js`; create `web/workspaces.js`, `web/tests/workspaces.spec.mjs`。

**Interfaces:** 保留 `BrowserAgent.create(...)` 默认行为；新增底层 `OpfsFileSystem::open_workspace(id: &str)` 与 JS `opfsOpenWorkspace(id)`，仅接受 default/UUID；`opfsListWorkspaces() -> Promise<string[]>` 仅扫描 Harness 管理目录。

- [ ] 写/保留 RED：绝对 `--session-name` 生成路径、显式会话路径和 list/show/delete 正常；会话保存中断不损坏旧文件；所有 FileSystem 测试实现补兼容方法，不更改测试意图。
- [ ] 修复 Native 会话路径适配：workspace 内绝对路径先明确转成相对路径；原本允许的显式外部 session 使用独立受限会话文件读写适配，不放宽通用 NativeFs 根约束。会话保存恢复同目录临时文件 + sync + 原子 persist，读取拒绝管道。
- [ ] 写 workspace RED：default 老文件不移动；UUID A/B 同路径内容、列表/图片/ZIP 不串；../、绝对路径、伪 UUID 失败，名称不拼路径；旧相对 tmp/ 文件仍可见。
- [ ] 实现根选择和合法目录扫描；此时不实现临时挂载。`WasmPlatform.root` 仅描述虚拟路径，不能以标签代替真正 fs 根。
- [ ] 跑 `cargo test --test platform --test workspace --test archive --test agent_core --test browser_provider`、`cargo test --bin i --test agent` 和 workspace 浏览器测试；除已声明历史 ignored 外 GREEN，提交本任务文件。

## Task 4: IndexedDB 状态仓库和原子迁移

**Files:** Create `web/state/{store,records,recovery}.js`, `web/tests/state.test.mjs`, `web/tests/state.spec.mjs`; modify `web/workspaces.js`。

**Interfaces:** `openStateStore() -> Promise<StateStore>` 打开 `harness_state` version=1；object stores: workspaces(id)、sessions(id, workspaceId index)、ui(key)。`store.load() -> Promise<{workspaces,sessions,ui}>`、`store.commitSession(snapshot,expectedRevision) -> Promise<newRevision>`、`store.commitUi(ui) -> Promise<void>`、`store.createWorkspace(record) -> Promise<void>`。`withWorkspaceLock(id,fn)` 使用 Web Locks，缺能力明确失败。

记录固定：Workspace `{id,name,createdAt,storageKind:'persistent'}`；Session `{id,workspaceId,name,connectionId,connectionIdentity,history,transcript,draft,status,revision,journal,updatedAt}`；ui `{key:'main',activeWorkspaceId,activeSessionId,openedSessionIds,panels,scrollPositions}`。transcript 只存纯数据，不存 DOM/HTML；每 Session JSON 快照限制 64 MiB，超限明确阻止新的持久化和副作用，不静默裁剪历史。

- [ ] 写 RED：事务中止无部分记录；expectedRevision 冲突拒绝覆盖；旧 i_workspaces 名称映射 default 时不假称旧目录已隔离；default 创建幂等；未知 schema/损坏 Session 隔离且其他记录仍可打开；失败信息不打印原文。
- [ ] 增加孤立目录恢复、数据库配额拒绝、Web Lock 两窗口竞争、存储名/显示名边界的浏览器测试；Node 仅测纯数据校验/恢复 reducer，IDB 使用真实浏览器而不是 mock 全部 API。
- [ ] 实现事务 resolve 仅在 oncomplete 后返回；目录先建、记录后提交；安全扫描孤立目录后由用户确认恢复名称，不删除目录。迁移保存 source/version 标记，失败可重试，不删除用户旧数据。
- [ ] 运行 Node+Chromium/WebKit 状态测试 GREEN；提交状态仓库任务文件。

## Task 5: Agent 历史导入导出和工具中断日志

**Files:** Create `src/browser/session.rs`, `tests/browser_session.rs`, `web/sessions.js`, `web/tests/recovery.spec.mjs`; modify `src/browser/{mod,runtime}.rs`, `web/state/recovery.js`, `web/tests/mock-server.mjs`。

**Interfaces:** `BrowserAgent.create_workspace(workspace_id,on_event,on_approval,on_download,on_checkpoint)`；`export_history() -> JSON string`；`import_history(json) -> Result<()>` 仅 idle 且 checked_history 通过；异步 on_checkpoint 收到 `{history,journal}` 后必须等待其 Promise 成功。JS `SessionController` 固定 id/workspaceId/Agent；`restoreSession(record,connection)` 先配置再导入已验证历史，禁止 configure 悄悄清掉恢复历史。

journal：完整批次未执行前存 `{requestId,baseHistory,assistant,calls:[{id,status:'pending'|'started'|'finished',result?}]}`；每个调用开始前存 started，完成后存真实结果；稳定 checkpoint 成功后清 journal。所有写操作 await；失败立即停止新工具。

- [ ] 写 RED：两个 Session 历史/输出不串，模型收到正确恢复 history；非法/孤立/重复工具 ID 历史拒绝；恢复连接缺失/identity 不匹配不发请求或抹旧历史。
- [ ] 实现 workspace 创建兼容旧 create，按 Session 回调路由；before_turn、before_tools、dispatch、checkpoint 与 final error 保存真实进度，不改 Native Agent 语义或复制另一个 Agent 循环。
- [ ] 用服务端闸门和 checkpoint 故障注入测试：文字首段/等待审批/写文件后结果提交前刷新，均不自动 HTTP 或重放；started 无结果恢复为明确未知 outcome，pending 恢复为未执行，不冒充成功。构造完整 tool pairing 后再交现有 checked_history 校验。
- [ ] 增加保存失败阻止下一副作用、已写文件不回滚、半截文本非完成历史、晚到事件不串 Session 的测试。先观察 RED，再实现并跑 Rust+浏览器 GREEN。
- [ ] 提交会话和恢复任务文件，记录可重复的故障窗口。

## Task 6: 终端式工作区/会话 UI

**Files:** Create `web/ui/{tabs,files,terminal,settings}.js`; refactor `web/app.js`, `web/index.html`, `web/style.css`, `web/i18n.js`; modify `web/tests/{agent,mobile,opfs}.spec.mjs`; create `web/tests/tabs.spec.mjs`。

**Interfaces:** app 仅负责初始化/编排，`SessionController` 持有 session data；`renderTabs(state,onSelect,onClose)`、`renderFiles(workspaceId,files)`、`renderTerminal(session)`、`openSettings()/closeSettings()`。按钮行为通过稳定 data-action/id 与可访问名称测试，不为绕旧测试伪造隐藏控件。

- [ ] 写 RED：顶部工作区+当前工作区会话 tabs、创建/关闭/重开、同工作区共享文件；切换保留草稿、滚动与输出；后台审批只能回 owner 操作；Download 按 workspace 与 session 标记链接；Clear 不删历史，重置先确认。
- [ ] 实现安全消息区、折叠工具、复制、真实文件区、发送/停止、会话历史选择，替换假 /resume（保留命令时也必须走真实 Session store）。全窗口仅一个任务，后台状态明确；跨窗口 revision 冲突提示而非覆盖。
- [ ] 实现设置四组、最少默认字段、relay 条件显示、统一保存状态、危险凭据操作、历史清理单独确认；新建连接不覆写旧会话历史，连接不匹配要求用户明确选择。
- [ ] 实现移动端 tabs 横滑/单列/16px 输入/40px 控件/安全区；Esc、焦点返回、aria-selected、屏幕阅读器状态。仅保存可序列化视图状态。
- [ ] 更新现有测试定位到真实新入口，保留全部安全/副作用断言；运行 tabs、agent、mobile 和 settings 用例 GREEN，提交 UI 任务文件。

## Task 7: 自动保存与完整刷新恢复验收

**Files:** Create `web/persistence.js`, `web/tests/restore.spec.mjs`; modify `web/app.js`, `web/sessions.js`, `web/ui/terminal.js`, `web/pwa.js`, `web/state/store.js`。

**Interfaces:** `PersistenceCoordinator.enqueue(sessionId,patch)` 合并草稿/滚动/展示 delta（150ms），`flush(sessionId) -> Promise<void>` 等待排队事务；结构修改和 Agent checkpoint 立即 await。会话 revision 更新由单队列维护；无 beforeunload 同步写假保证。

- [ ] 写 RED：完成会话后不手动保存便刷新，恢复 active workspace/session/tab 顺序、每会话上下文/输出/草稿/滚动；下次请求确实包含旧消息；文件字节/ZIP 一致。
- [ ] 实现启动顺序 store→迁移→workspace→Agent→restore UI→load connection，无自动模型请求；恢复时 running/approval→interrupted，更新状态与 journal，旧审批授权无效。失效 object URL 由可导出文件重新生成，不恢复旧 URL 字符串。
- [ ] 保存状态只在事务成功后显示；quota/拒绝明确提示且阻止新工具，普通阅读和文件备份仍可用；页面刷新恢复最后成功快照，突然终止可能丢失未提交展示片段的限制明确说明。
- [ ] PWA 升级前 flush 并检查活动任务；保存失败不激活更新。真实多窗口验证不同会话保存不串，冲突不覆盖；新 profile 无其他 profile 的凭据，清凭据不删历史文件，单独清历史不删文件。
- [ ] 在 Chromium/WebKit 跑 restore/recovery/多窗口/PWA 定向回归 GREEN，提交持久化任务文件。

## Task 8: 构建、文档、完整回归与独立审查

**Files:** Modify `scripts/{build_web.sh,build_pwa.mjs}`, `docs/wasm.md`, `README.md`, `README.zh-CN.md`, `web/tests/pwa.test.mjs`；必要时为新增模块补构建引用依赖检查。

- [ ] 写 RED：发布白名单覆盖所有本地 ES module 依赖，含子目录 UI/state、i18n、WASM snippets；未知资源/API 不被缓存或伪装成 HTML；持久记录与凭据绝不进入 Cache Storage。
- [ ] 实现可复现资产收集与哈希，子路径部署；不包含 tests/node_modules/配置/会话，完整构建原子发布。文档说明本机保存、配额/回收、最后提交快照、恢复不重试、连接测试可能计费及预览/tmp 后续边界。
- [ ] 跑 `sh scripts/build_web.sh`、`npm --prefix web test`、`npm --prefix web run test:browser`、`npm --prefix server test`、`cargo check --lib --target wasm32-unknown-unknown --no-default-features --features wasm`、`cargo test --bin i --test agent --test platform --test workspace --test archive --test agent_core --test browser_provider --test browser_session`；记录每套结果及历史 ignored，不声称已验证真实手机或付费 API。
- [ ] 一次独立最终审查覆盖整体 diff 与用户基线；Important/Critical 必须有 RED→GREEN 修复并重跑相关完整套件，Minor 明确记录。不自动装插件、公开服务、合并或推送。
- [ ] 提交本轮文件，保留用户无关改动，给出启动方式、测试证据、已知限制与下一阶段范围。

## 审阅与执行方式

自审：spec 每节均映射到以上任务；新接口相互一致；恢复与安全测试先于行为修改。执行方式沿用主代理当前会话实现、最后独立审查。请用户审阅此实施计划；确认前不开始产品实现。
