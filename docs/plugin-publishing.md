# 发布浏览器插件到社区目录

[插件目录](https://i.lninl.com/plugins/)使用独立的
[qinains/i-plugins 注册表](https://github.com/qinains/i-plugins)，采用独立的社区 PR
审核流程。插件作者拥有并维护自己的源码仓库、固定源码提交和 release。
先按照[插件开发教程](plugin-development.md)构建 `.hplugin.zip`，再提交注册表 PR。
桌面 NDJSON worker 不属于此目录。

## 提交固定版本

1. 在你维护的 GitHub 仓库提交源码，记录完整的 40 位小写十六进制 commit SHA。
2. 使用官方打包器构建 `.hplugin.zip`，创建固定版本的 GitHub release 并上传插件包。
3. 计算整个 ZIP 文件的 SHA-256 和字节数。ZIP 的 SHA-256 不等于清单中的 WASM 模块散列。
4. 按注册表仓库的贡献说明提交源码条目 PR，填写源码仓库、固定提交、release 包和显式权限。维护者审核并由构建流程校验、镜像插件包。
5. PR 合并后注册表自动重建并发布 GitHub Pages；官网与工作台下次加载目录时即可看到。无需等待主应用发布。

不能替换已发布版本的内容或散列。修改代码、清单、许可或 ZIP 内容时发布新版本、
新源码提交和新包散列；不要复用同一版本或依赖可变分支、标签指向。

## 源码条目与公开目录

注册表 PR 新增 `extensions/<id>.json`，例如 `extensions/example.word-count.json`。
源码条目如下，填写作者控制的 GitHub release 下载地址；不要伪造镜像地址。
构建后所有字段保持不变，只有 `package.url` 被改写为受信任的内容寻址镜像地址。
完整公开目录根对象为 `{"schema_version":1,"plugins":[...]}`。

```json
{
  "id": "example.word-count",
  "name": "Word count",
  "version": "1.0.0",
  "description": "Count words in a workspace file",
  "repository": "https://github.com/example/word-count",
  "source_revision": "0123456789abcdef0123456789abcdef01234567",
  "license": "MIT",
  "package": {
    "url": "https://github.com/example/word-count/releases/download/v1.0.0/example.word-count-1.0.0.hplugin.zip",
    "sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    "size": 12345
  },
  "permissions": {"workspace": "read", "storage": false}
}
```

示例散列、字节数和仓库仅为格式说明，不是可安装插件。

- `id`、`name`、`version` 标识插件，必须与插件包清单一致；`description` 描述插件用途。
- `repository` 为作者的 HTTPS GitHub 源码仓库；`source_revision` 固定到完整 40 位 commit SHA。
- `license` 声明插件许可；包内仍须包含许可文件。
- 源码条目的 `package.url` 必须是上述同一 GitHub 仓库的 release 下载地址。公开目录的 `package.url` 只能是 `https://qinains.github.io/i-plugins/packages/<sha256>.hplugin.zip`。
- `package.sha256` 是完整 ZIP 的 64 位小写 SHA-256；`package.size` 是实际 ZIP 字节数。
- `permissions.workspace` 必须显式为 `none`、`read` 或 `read-write`；`permissions.storage` 必须显式为布尔值。目录不能通过省略字段扩大权限，声明必须与包内清单一致。

目录地址为 `https://qinains.github.io/i-plugins/index.json`，格式版本为 `schema_version: 1`。
目录加载不发送身份凭据或 Referer，响应限制为 512 KiB、30 秒；下载的包仍须独立校验散列、
大小、清单和权限。目录与包校验失败时应停止，不从不受信任地址回退下载。

注册表 CI 使用固定版本的官方验证器，静态检查完整 ZIP、WASM 格式与 ABI，不执行插件。
提交前也可在主项目源码目录运行相同检查：

```sh
cargo run --locked -p i-wasm-package --features cli --bin i-plugin-check -- path/to/plugin.hplugin.zip
```

## 安装与信任边界

官网只展示元数据和下载链接，不执行、安装或启用插件。进入工作台的插件设置，加载目录，
下载并检查插件包，然后明确确认安装及其权限。收录和审核不代表验证发布者身份或提供安全保证。
目录刷新、PR 合并或新版本发布都不会自动安装、启用、授权或更新插件；新包始终需要重新检查确认。
