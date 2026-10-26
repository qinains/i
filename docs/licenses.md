# 开源许可与第三方声明

i / lninl 的自有代码使用仓库根目录的 MIT License。第三方代码、字体数据和依赖保留
各自的版权及许可，不因本项目采用 MIT 而重新许可。

`THIRD_PARTY_LICENSES.txt` 汇集 Cargo.lock 所锁定依赖的上游许可和版权文本，按内容
哈希去重；它是构建、测试和目标平台依赖的保守超集，不表示每个分发产物都链接了
全部列出的依赖。[下载完整文本](https://i.lninl.com/third-party-licenses.txt)。

globset 使用 crates.io 官方依赖，其上游许可保留在 `THIRD_PARTY_LICENSES.txt` 中；仓库不再维护本地修改版。
JavaScript 模块词法分析器的许可及声明分别位于 `web/module-lexer.LICENSE` 与
`web/module-lexer.NOTICE`，随浏览器工作台一并分发。

## 维护

```sh
cargo fetch --locked
python3 scripts/build_license_notices.py
python3 scripts/build_license_notices.py --check
python3 -m unittest discover -s scripts -p 'test_*.py'
```

收集器只读取锁定依赖源码中的许可文件，不执行它们。少数上游 crate 没有打包其
仓库级许可，因此 `vendor/licenses/index.json` 记录所用上游 revision、文件与 SHA-256。
这些补充文本随仓库保存，普通构建与校验不需要 GitHub 凭据或联网获取补充声明。
`--fetch-missing` 是维护者显式操作：仅查询代码中批准的公开上游仓库，并使用 crate
携带的固定 VCS revision；新增来源必须人工审查。

升级依赖后应重新生成、查看 diff，核对上游许可变化，再提交更新。不能仅靠 SPDX
标识推断全部许可义务已满足，也不要删除上游版权、NOTICE 或许可文本来减小体积。

WASM/PWA 和原生包都携带项目 MIT 与完整第三方文本。浏览器扩展没有 Rust 链接依赖，
其源码及许可保留在下载 ZIP 内。Safari 工程中的 Apple 模板代码仍保留原始说明。

`scripts/build_wasm_plugin.sh` 会在插件自己的 LICENSE 后合并其锁定依赖声明及 SDK
的 MIT 文本，再装入插件 ZIP 的 LICENSE 文件。插件作者仍需核对自己代码及依赖的
许可要求；不能把整个插件自动标为 MIT。包格式的 LICENSE 上限为 256 KiB，超限会
明确失败，不会静默删除版权文本。
