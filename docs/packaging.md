# 原生打包与发布检查

目前的安装渠道是仓库源码和已公开的 [GitHub Release 附件](https://github.com/qinains/i/releases)。不据此宣称已有 Homebrew、Winget 或 crates.io 安装包；不要把模板命令 `cargo install i` 当作已发布的安装方式。

主程序和可选连接插件分别打包。前者是 bundled `i`，后者是 ChatGPT/Claude Console 本地连接程序，不是工作区 WASM 插件。连接插件的下载、解压和配对步骤见 [连接插件安装](chatgpt-login.md)。

## 主程序：构建与打包

从仓库根目录执行，需要 Rust 工具链及目标平台的构建依赖；原生工具不依赖解释器，前端构建与测试另需 Node.js。

```sh
cargo run --locked --features packaging --bin i-package -- --profile min-size
```

也可先 `cargo build --locked --features packaging --bin i-package`，再运行 `target/debug/i-package`；Windows 使用 `.exe`。`scripts/package.sh` 是 Rust 工具的便捷包装，不包含另一套打包实现。

| 参数 | 含义 |
| --- | --- |
| `--profile release` | 标准 release 优化 |
| `--profile min-size` | 默认配置，体积优化、LTO、去符号等，保留功能 |
| `--skip-build` | 使用仓库 `target/<profile>/i`，Windows 为 `i.exe`；由调用者保证是正确的 bundled 构建 |
| `--outline` | 仅 AArch64、`min-size` 且不使用 `--skip-build` 时，增加 LLVM machine outlining 参数 |

普通打包会从 `rustc -vV` 获取本机 host，以 `--features bundled --bin i --profile PROFILE --target HOST` 构建到仓库 `target/HOST/PROFILE`。它不是任意目标的交叉编译器，也不会一次构建全部平台。`--outline` 不仅限于 macOS，但其可用性仍需对应工具链实际验证。

使用已构建的主程序时：

```sh
cargo build --locked --features bundled --bin i --profile min-size
cargo run --locked --features packaging --bin i-package -- --profile min-size --skip-build
```

`--skip-build` 查找 `target/min-size/i`，**不是** `target/<host>/min-size/i`。自定义 `CARGO_TARGET_DIR` 或显式 `--target` 的构建应注意这个差别，不要误打包旧文件。打包器自身也应从仓库内的 target 目录运行，以便定位源码和许可文件。

## 主程序产物

归档名为 `i-<version>-<host>-<profile>`：macOS/Linux 同时生成 `.tar.gz` 和 `.tar.xz`，Windows 生成 `.zip`。两种 tar 使用相同内容，压缩均由 Rust 库完成；每个归档有相邻 `.sha256`。临时打包目录完成后清理，`dist/` 中不保证保留一份已解压目录。

```text
i-<version>-<host>-<profile>/
  bin/i                         # Windows: bin/i.exe；只有这一个主程序
  config.example.json           # 不含个人凭据的示例，不是用户全局配置
  README.txt
  start.sh                      # Unix 便捷入口；Windows 直接运行 bin/i.exe
  logo.svg
  LICENSE.txt
  THIRD_PARTY_LICENSES.txt
  BUILD.json
  SHA256SUMS
```

`BUILD.json` 记录版本、目标、profile、bundled 布局、rustc、额外编译参数、字节数和已知限制。`SHA256SUMS` 覆盖包内其他文件，包括 BUILD 元数据和许可。globset 使用官方 crates.io 依赖，其许可汇入 `THIRD_PARTY_LICENSES.txt`，没有单独的 globset 许可证产物。

Bundled 把可选运行时与 worker 链接进同一个 `i`，通过 `i --runtime MODULE` 启动，不要求旁置 `i-plugins`。**独立连接插件不包含在该二进制或主程序归档内。** 分发程序不需要 Cargo、源码仓库或 Node.js，但仍需要目标平台的系统库；具体 TLS/凭据存储依赖见 [部署说明](deployment.md)。

## 可选 ChatGPT / Claude 连接包

```sh
cargo run --locked --manifest-path tools/native-tool/Cargo.toml --target-dir target -- package-connections --profile min-size --verify
```

该命令独立构建 `i-chatgpt` 包的两个程序，输出 `dist/i-connections-<host>.zip` 和 `.zip.sha256`。连接包不在文件名中编码版本，应从同一个 Release 取得附件及摘要，升级时使用新的目录，不混用不同版本的程序和文档。

```text
bin/i-chatgpt-plugin             # Windows: .exe
bin/i-claude-plugin              # Windows: .exe
BUILD.json
README.txt
LICENSE
THIRD_PARTY_LICENSES.txt
```

`--profile` 可取 `debug`、`release` 或 `min-size`；`--skip-build` 使用仓库 `target/PROFILE` 中的现有两个程序。已有同名 ZIP 或摘要时会拒绝覆盖；为新的构建先选择并核对新的发布工作目录，不把旧包当作本次产物。`--verify` 解压后以隔离的本地状态调用两个程序的 `manifest`，不登录真实账户，也不证明实际 OAuth/计费已验证。

Claude 登录免填 API key，但通过官方 `ant` 使用 **Claude Console API 计费**，不是 Pro/Max 订阅额度；连接包不捆绑 `ant`。ChatGPT 套餐使用需要 OpenAI 的资格与授权，不能承诺免费或无限调用。

## 离线验证与许可

主程序归档检查命令：

```sh
cargo run --locked --manifest-path tools/native-tool/Cargo.toml --target-dir target -- check-package 'dist/*.tar.gz' 'dist/*.tar.xz'
# Windows 使用实际 ZIP 文件名或相应模式，不匹配不存在的 tar 格式：
cargo run --locked --manifest-path tools/native-tool/Cargo.toml --target-dir target -- check-package 'dist/i-*-min-size.zip'
```

模式由工具展开，每个模式都必须匹配现有文件。不要把 `i-connections-*.zip` 传入主程序检查器：它要求单个 bundled 主程序的布局。检查器校验安全解压路径、项目和第三方许可、文件/归档摘要、BUILD 布局与大小，并实际检查初始化、安装后搬移、Unicode 搜索、控制台 mock 模型循环、memory 和 cache；不是只看压缩包里是否有一个文件。

生成或更新许可后再打包，防止分发过期文本：

```sh
cargo run --locked --manifest-path tools/repo-tool/Cargo.toml --target-dir target -- licenses
cargo run --locked --manifest-path tools/repo-tool/Cargo.toml --target-dir target -- licenses --check
cargo test --locked --features bundled,packaging --bin i-package
```

许可收集器覆盖主工作区和独立工具工作区的完整锁文件，需要已缓存相应依赖；缺失会失败，不通过删条目来缩小产物。详情见 [第三方声明维护](licenses.md)。摘要不匹配时应重新获取或重新构建产物并核查来源，不能仅为让检查通过而重写摘要。

## 发布与安装边界

仓库的 `v*` tag 发布工作流先执行安全、原生和浏览器检查，在各 runner 的**本机架构**打包并检查主程序与连接插件，再上传附件并创建 **draft Release**。工作流成功不等于附件已经公开；发布者仍需审核目标、摘要、许可、说明和已知限制，再显式公开 Release。使用已配置的工作流，不另抄一套未验证的发布 YAML。

源码安装主程序可用：

```sh
cargo install --path . --locked --features bundled --bin i
```

Release 使用者核对平台和 CPU 后解压归档，从包内 `bin/i`（Windows 为 `bin/i.exe`）启动。macOS 包不是 Developer-ID 签名或公证版本，不能把 SHA-256 当作代码签名或绕过系统安全审批的理由。包内 BUILD 记录的是实际目标，不是通用二进制承诺。

本文描述当前实现与发布流程，不声称真实账户登录、所有平台本机运行或某个版本的附件已公开可用。构建和发布验收应记录当次实际输出，不能用历史体积、mock 测试或单平台成功代替其他平台的验证。
