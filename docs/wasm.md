# 浏览器 WASM

## 构建与启动

需要 Rust 1.91+、`wasm32-unknown-unknown`、wasm-pack 和 Node.js 24+（构建静态 PWA）；纯直连的 Web 运行时不需要 npm 或服务端 Agent。

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-pack --locked
./scripts/build_web.sh
python3 -m http.server 8080 --bind 127.0.0.1 --directory web
```

打开 `http://127.0.0.1:8080`。必须通过 localhost 或 HTTPS 加载，不支持双击 `file://`。构建输出为 `web/pkg/`、`web/assets.json`、`web/service-worker.js` 和 PNG 图标（均不提交生成文件）；脚本禁用额外 wasm-opt 步骤，避免隐式下载优化器，Rust release 优化仍启用。静态发布应只部署 assets.json 中的 assets 加 assets.json，不暴露 tests/node_modules 或仓库文件；支持根路径或 `/harness/` 等子路径，必须原子发布整个构建，不能混用不同版本的 JS/WASM。

原生入口仍为 `cargo run --bin i -- ...`。普通 `cargo build` 只构建根 package；需要 worker 时先 `cargo build -p i-plugins`，或 `cargo build --workspace`。WASM 通过 library 的 `cdylib` 导出，不再使用占位的 i-wasm binary。

## 模型配置与任务

填写 Base URL（如自己配置的服务 `/v1` 根）、模型和 API key。页面自动补一次 `/chat/completions`。浏览器首期只支持 OpenAI-compatible Chat Completions，不支持 Responses、Anthropic 或原生进程 provider/plugin；原生已有协议不变。

服务需要允许页面 origin 的 CORS、POST、`Content-Type` 和（设置密钥时）`Authorization`。HTTPS 页面不能请求被浏览器混合内容规则禁止的 HTTP endpoint。没有自动代理或 CORS 绕过。网络失败显示错误，不自动重试整个回合。

默认使用 `max_tokens`；服务要求 `max_completion_tokens` 时取消对应复选框。不按模型名猜测。默认每任务最多 32 个模型回合，可调为 1..32。输出 token 上限必须大于零。

默认 API key 仅在页面内存中使用，刷新后重新输入。勾选 **Remember this browser's configuration (including API key)** 后，整套浏览器 Harness 配置会自动保存到当前 origin 的 localStorage，记录名为 `i_harness_config`、版本为 2：API URL、模型、API key、协议、步数、token 上限、token 参数选项、语言、transport、stream 和 relay token。旧 version=1 迁移为 direct + stream，不自动发请求或更换 key 接收方。下次打开或刷新自动恢复；修改表单也自动更新，不必先运行任务。

取消勾选会删除该记录，当前页面的值仍可继续使用；**Clear configuration** 同时删除保存记录、清空当前表单和活动模型配置/历史，但不删除 OPFS 文件。语言偏好兼容旧 `i_lang` 记录，清除配置时也删除它。浏览器阻止存储、配额不足、JSON 损坏或版本不兼容时显示状态，不伪称保存成功。

localStorage 不是加密保险箱，同源脚本可以读取已保存 key，不应在共享设备上开启。清除站点数据也会删除配置。配置/密钥不写 OPFS、会话或 ZIP；不要自行把密钥写入上传文件或任务文本。服务 URL 不允许内嵌用户名/密码、query 或 fragment。切换 URL/model 开启新模型会话。

默认开启 Stream，首段到达即显示；不兼容 SSE 的接口可手动关闭后重试，不自动重复请求。完整结束标记、结束原因与参数通过校验前不执行任何工具；取消或断流保留文字并标记 incomplete，但不把半截回复作为完成历史。请求最多 180 秒。write/edit 需要点击 Allow；Deny 会作为工具错误返回模型。Cancel 中止请求及后续工具，已完成的文件写入不回滚。New session 清模型历史而不删除文件；Clear 只清显示。

## PWA、跨域与 PC 插件

默认 Direct 模式直接访问支持 CORS 的模型接口，不需要服务器或插件。PWA 不解除 CORS；网络 TypeError 可能是 CORS、网络或地址问题，需查看浏览器开发者工具确认，不能据此判断 key 无效。桌面 Chrome/Edge 用户可自行安装可信 CORS 插件，并将权限限制为当前页面和选定 API，测试完关闭；本页面不静默安装插件、不关闭全局安全设置。iPhone 不依赖桌面插件。

不支持直连的接口可显式选择 Own same-origin relay，并部署[独立可选服务](../server/README.md)。模型 key 会经过该自有服务，relay token 只发送页面同源 API，不支持输入任意第三方代理地址。直连失败不自动切换，不部署 relay 也不影响静态应用使用。

iPhone：用 Safari 打开可访问的 HTTPS 地址，分享 → 添加到主屏幕。其他浏览器支持安装提示时显示 Install app，否则使用手动指南。第一次成功在线缓存后，离线可启动 WASM、访问 OPFS、上传与导出 ZIP；AI 仍需联网。不要假设安装态与普通标签页共享配置/文件，首次打开以实际状态为准。锁屏或后台可能暂停请求，不承诺持续生成。

Service Worker 只缓存构建白名单静态资源，逐项校验 SHA-256，不缓存模型 POST、响应、配置或密钥。更新提示出现后，用户点击 Apply app update；所有标签页空闲并确认锁定后才激活和刷新，生成/审批中延后更新。缺资源或版本不一致时安装失败，旧版本继续可用。同源根路径与子路径同时安装等情况下，浏览器可能不向等待 worker 递送消息；页面会明确显示 postponed，按提示关闭所有应用标签页/窗口后重新打开，不伪称已更新。不同注册不会互相锁住 UI。只清理本应用 scope 的旧缓存，不删 OPFS 或配置；浏览器回收/清站点数据仍可能删文件，应定期导出 ZIP。

## 文件与下载

OPFS 在当前 origin 私有根目录的 `workspace` 下保存文件，界面显示为 `/workspace`。支持二进制上传、嵌套目录、Unicode 名称，以及 read/write/edit/view_image/list。read 的行号从 1 开始；edit 必须唯一匹配，可提供 line_number。浏览器 shell 在 schema、输入和执行层均禁用。

文件读写每文件最多 8 MiB，图片最多 8 MiB，每批最多 8 张；工具文本前缀预算 1 MiB。已知轻微差异：截断标记还会增加至多 12 字节。模型 HTTP 请求/响应上限 48 MiB。

Upload 写入成功并关闭 writable 后才报告成功；重名需确认。OPFS 不可用、配额不足或写入失败不会回退为假持久化内存文件。刷新后文件保留，但模型历史不会跨刷新自动恢复。存储按 origin 隔离：更换协议/主机/端口会看到不同工作区；清除站点数据会删除文件，浏览器也可能回收非持久存储，应及时下载备份。

按钮 Download 与模型工具 `download({paths?, filename?})` 共用 Rust ZIP 实现。默认递归导出全部 workspace；paths 可选择文件或目录；filename 默认 `workspace.zip`，必须是无路径的 `.zip` 名称。保留目录、空目录和二进制字节；stored ZIP 不压缩内容。单次最多 64 MiB 文件内容、10000 个条目，超限失败而不是生成截断 ZIP。

模型调用 download 会生成持久可点击的 Save 链接，用户点击后保存；工具的「ZIP ready」不代表文件已经存入本机。按钮也保留该链接，便于浏览器拦截自动下载时手动保存。ZIP 默认不含终端显示、配置或 API key，只含用户工作区文件。

原生 download 默认写 `.i/downloads/`，默认全工作区导出排除该自动产物目录；覆盖必须确认。输出不得与显式源路径重合，拒绝符号链接输出及其符号链接祖先，并使用同目录临时文件原子替换，避免写穿硬链接源文件。

## 验证与边界

```sh
cargo build -p i-plugins
cargo test --workspace
cargo check --lib --target wasm32-unknown-unknown --no-default-features --features wasm
./scripts/build_web.sh
npm --prefix web ci
npm --prefix web exec -- playwright install chromium webkit
npm --prefix web test
npm --prefix web run test:browser
npm --prefix server test
```

Web 测试启动 localhost 静态服务与 mock API，不依赖付费模型。真实 Chromium 覆盖模型工具审批/拒绝、OPFS 持久化、二进制上传、按钮/工具 ZIP 下载并通过独立 unzip 解压、取消晚到响应、IME Enter、截断批次、CORS 失败和 OPFS 不可用。共享 Rust 测试覆盖路径保护、原生写入兼容、ZIP 稳定性/限额和输出别名保护。

原生继承代码的 `tests/network.rs` 仍有 4 项失败，全部已在本次修改前保存的源码中复现：

- `disconnect_after_tool_retries_identical_request_without_replaying_tool_or_hiding_usage`
- `empty_proxy_overrides_environment`
- `invalid_proxy_fails_without_sending_the_model_request`
- `socks5h_and_http_connect_proxies_route_without_local_target_dns`

因此不宣称 `cargo test --workspace` 全绿。旧 `all_core_production_rust_stays_under_1024_lines` 测试在原始代码中已经失败；新增共享核心/WASM 后不再以 1024 行为目标，保留为显式 ignored 的历史约束，不通过挪文件隐瞒代码。

源码统计（包含空行/注释与文件内测试，排除独立 `src/tests.rs`，包含 packaging/completions）：

```sh
find src -name '*.rs' ! -path src/tests.rs -print0 | xargs -0 wc -l
```

源码数量会随实现变化，以上命令可复算；另有 `plugins/runtime`、vendor、JS、JSON、测试等，不能把 src 行数当完整客户端总量。旧原生发布体积测量不适用于当前重构；本次未重测原生发布体积。测试不使用真实付费模型；桌面 WebKit/手机 viewport 测试不等于真实 iPhone、Safari 安装流程或 Windows 实机验证。

参考：[MDN OPFS 入口与安全上下文](https://developer.mozilla.org/en-US/docs/Web/API/StorageManager/getDirectory)、[wasm-pack Web 构建](https://wasm-bindgen.github.io/wasm-pack/book/commands/build.html)。
