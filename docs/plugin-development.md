# 从零编写 Rust 插件

本教程产出两个不同的插件：浏览器 `.hplugin.zip` 和桌面 NDJSON worker。
都不需要真实 API key 即可构建和测试。CORS 浏览器扩展不属于此教程的工具插件。

## 准备环境

从源码仓库根目录操作，要求 Rust 1.91+、Cargo、Node.js、Python 3。浏览器测试
还需要 npm 依赖和 Playwright 浏览器。首次为浏览器插件安装 target：

```sh
git clone https://github.com/qinains/i.git
cd i
rustup target add wasm32-unknown-unknown
```

SDK 当前使用仓库内 path dependency，不承诺 crates.io 已发布同名 crate。

## 教程一：浏览器统计文件字数

### 1. 先运行完整示例

```sh
sh scripts/build_wasm_plugin.sh examples/wasm-word-count dist/plugins
```

应生成 `dist/plugins/example.word-count-1.0.0.hplugin.zip`。
脚本声明最大内存、编译、复制许可，并用官方打包器校验模块、更新散列。
不要把整个 crate 目录 ZIP 后当作插件。

### 2. 看清 Rust 与清单如何配对

`examples/wasm-word-count/Cargo.toml` 使用 `cdylib` 和仓库 SDK：

```toml
[lib]
crate-type = ["cdylib"]

[dependencies]
i-wasm-sdk = { path = "../../plugins/wasm-sdk" }

[profile.release]
panic = "abort"
opt-level = "z"
lto = true
strip = true
```

完整逻辑在 `examples/wasm-word-count/src/lib.rs`：

```rust
use i_wasm_sdk::*;
#[derive(Default)]
pub struct WordCount;
impl Plugin for WordCount {
    fn start(&mut self, request: ToolRequest) -> StepOutput {
        if request.data.name != "count_words" {
            return StepOutput::Error { error: "unknown tool".into() };
        }
        match request.data.arguments["path"].as_str() {
            Some(path) => host_read(1, path),
            None => StepOutput::Error { error: "path required".into() },
        }
    }
    fn resume(&mut self, response: HostResponse) -> StepOutput {
        if !response.ok {
            return StepOutput::Error { error: response.error.unwrap_or_else(|| "read failed".into()) };
        }
        let result = response.result.unwrap_or(Value::Null);
        match result["text"].as_str() {
            Some(text) => StepOutput::Done {
                output: json!({"words":text.split_whitespace().count(),"characters":text.chars().count()}).to_string(),
            },
            None => StepOutput::Error { error: "invalid text response".into() },
        }
    }
}
export_plugin!(crate::WordCount);
```

插件不直接打开文件：先返回 `fs.read`，宿主检查当前工作区和 `workspace:read`
权限，再返回文本给 `resume`。工具 schema 和本地名称来自 `manifest.json`。
它统计的是空白分隔词与 Unicode scalar 数，不是中文分词或模型 token 数。

### 3. 在浏览器安装和验证

```sh
sh scripts/build_web.sh
cd web
npm ci
node serve.mjs
```

打开终端输出的 localhost 地址。设置 → 插件 → 选择 `.hplugin.zip`，检查权限后
为当前持久工作区启用。上传 `notes.txt`，内容为 `hello world`。
配置自己的模型后可要求调用统计工具，预期 `words:2,characters:11`。
这一步会调用服务商；不要把自己的 key 发进 issue 或测试日志。

不使用模型的自动验证：

```sh
sh scripts/build_plugin_fixtures.sh
cargo test -p i-wasm-sdk -p i-wasm-protocol -p i-wasm-package
cd web
npx playwright install chromium webkit
npm run test:browser -- tests/plugin-agent.spec.mjs tests/plugin-broker.spec.mjs
```

这些浏览器测试使用 mock 模型和真实 WASM / OPFS，不证明真实账号的接口权限。

### 4. 改成自己的插件

复制 `examples/wasm-word-count` 到同层新目录，改 crate 名、插件 ID、显示名、工具名
及 schema。保持 SDK 相对路径有效。只需要纯计算时设 `workspace:none,storage:false`；
写文件才申请 `read-write`；需要持久私有状态才申请 storage。
参考 `examples/wasm-json-format` 的纯计算模式。每次发布递增版本并重新打包。

测试至少包含：正常输入、缺字段/错类型、未知工具、拒绝写入、路径逃逸、二进制文件、
超限、错误 host ID、超时/取消、重新加载与工作区隔离。完整接口见
[WASM 插件参考](wasm-plugin-reference.md)。

## 教程二：桌面常驻 NDJSON worker

```sh
cargo build --manifest-path examples/rust-plugin/Cargo.toml --release
printf '%s\n' '{"id":1,"event":"tool","workspace":"/tmp","data":{"name":"rust_word_count","arguments":{"text":"hello world"}}}' | examples/rust-plugin/target/release/i-plugin-example
```

预期是一行 JSON，ID 为 1，`data.output` 是含 `words:2,characters:11` 的 JSON 文本。
stdout 只用于协议；每帧换行、flush，日志写 stderr。错误返回同 ID 的 error 字符串。
超时或错误后不透明重放，因为 worker 可能已经产生副作用。

注册示例是 `examples/rust-plugin.json`，其相对 exec 按清单目录解析。源码运行：

```sh
cargo run --features bundled --bin i -- --plugins examples/rust-plugin.json -C /path/to/project
```

无完整连接配置时会先配置或明确报错；独立 core-only 使用者还需要 provider 绑定。
生产清单建议使用明确的绝对 exec/cwd，Windows 替代文件为
`examples/rust-plugin.windows.json`。不要用 Shell 拼接未信任参数。

原生插件可以使用 OS 能力，属于可信程序，不具备 WASM 沙箱隔离。hook 会自动执行，
必须审查来源；工具仍经过宿主审批。完整生命周期和帧限制见 [worker 契约](plugins.md)。

## 提交 PR 的最小材料

源代码、许可、manifest、可复现构建命令、synthetic 输入输出、权限说明、测试证据
与失败后的副作用说明。不要提交 target、真实文件、key 或模型对话。
详见 [CONTRIBUTING](../CONTRIBUTING.md)。
