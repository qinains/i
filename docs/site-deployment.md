# 官网构建与部署

生产域名为 `https://i.lninl.com`，官网和文档位于根目录，浏览器工作台在 `/app/`。
整个产物是静态文件，可以部署到 GitHub Pages 或 Caddy，不需要运行模型后端。
最终采用 GitHub Pages；Caddy 服务器保留给可选 Relay，不因发布官网而修改现有站点。

## 本地构建

```sh
sh scripts/build_web.sh
python3 scripts/build_site.py
python3 -m unittest discover -s site -p test_build.py
python3 -m http.server 8090 --directory dist/site
```

打开 `http://localhost:8090/`，工作台为 `/app/`。构建器校验 app 静态资源的 SHA-256，
只复制 `web/assets.json` 明确列出的资源，不发布 tests、node_modules、源码仓库或
本地数据。用户文档取自顶层 `docs/*.md` 与公开贡献/许可文档，设计过程和临时验证
记录不生成官网页面。无需外部字体、CDN、分析脚本或后端搜索服务。

## GitHub Pages

工作流 `.github/workflows/pages.yml` 在默认分支构建并发布 `dist/site`。Pull request
可以构建检查，但不部署。配置 repository Settings → Pages → GitHub Actions。
再绑定 Custom domain 为 `i.lninl.com`，最后在 DNS 提供商配置：

```text
Type: CNAME
Name: i
Value: qinains.github.io
```

不要包含 `/i` 或仓库 URL。删除同名冲突的 A/AAAA 记录；不要为所有子域名使用通配
配置。绑定域名并正确解析后启用 Enforce HTTPS。GitHub 证书签发和 DNS 缓存可能
需要等待，不能以 Actions 构建成功代替公开 HTTPS 验证。

只有仓库构建和发布所需的 GitHub Token 权限；不需要把 API key、SSH key、DNS token
或服务器口令放入仓库。浏览器模型连接由最终使用者自行配置。

## 可选 Caddy 部署

若以后切换回自有服务器，上传完整构建到新的 release 目录，切换 `current` 软链，
保留前一个版本用于回滚。先备份 Caddy 配置，再添加独立站点并 validate 后 reload：

```caddyfile
i.lninl.com {
    root * /srv/lninl/current
    encode zstd gzip
    @wasm path *.wasm
    header @wasm Content-Type application/wasm
    file_server
}
```

不要覆盖无关站点，也不要把仓库根目录设为 Web root。Relay 是另外的授权服务，
如果站点托管在 GitHub Pages，不能假设它自动拥有同源 `/app/api/chat/completions` 路由。
自建时可运行独立的 `i-relay` 中继程序，不需要 Node 服务；中继只转发 API，不提供静态文件。
完整环境变量、安全限制及 Caddy 路由示例见 [中继部署](../server/README.md)。

## 发布后验收

首页下载专区通过 `site/releases.json` 列出已核对的原生发布包，不在浏览器运行时调用
GitHub API。发布新版本时，先等待三个平台构建及包内离线测试通过，检查归档中的
`BUILD.json` 与 `SHA256SUMS`，再对照下载归档及相邻 `.sha256` 的实际摘要更新清单。
清单只接受约定的平台、target、文件名和 64 位摘要；无发布记录的客户端显示发布进度，
不生成假下载链接。`v*` tag 工作流先创建草稿 Release，验收后再显式公开。

首页与共享导航支持简体中文、繁体中文、英文、日文、韩文、法文、德文。
首次按 `navigator.languages` 选择；手动选择保存在 `lninl_site_language`，
主题保存在 `lninl_site_theme`。它们不读取工作台的连接或密钥配置。
技术文档不会自动机器翻译，正文保留源语言标记；首页明确提示这一范围。
交互工作台只是前端示例，不会发起模型请求。减少动态效果偏好会禁用入场和光标动画。

Logo 中间 `i` 统一使用 `#22d3ee`；官网 favicon 与工作台 SVG 保持一致。
`build_pwa.mjs` 生成同配色的 192/512 PNG。更改图标时需同时更新几何栅格化及品牌测试，
重新构建 PWA；已安装应用的桌面图标更新时间由浏览器决定，不要清除工作区数据。

首页、文档、站内搜索、移动布局、MIT/贡献页面、三套扩展下载、`/app/`、WASM MIME、
PWA scope 与离线资源均需检查。跨托管平台但保留相同 HTTPS origin 时，浏览器
存储身份仍由 origin 决定；不要为“更新”而清除用户站点数据。

GitHub Pages 不提供自定义服务器安全响应头；官网使用 CSP meta，工作台保留自身
预览安全边界。不要声称静态托管提供了任意后端、扩展签名或跨域代理能力。

[GitHub 自定义子域名文档](https://docs.github.com/pages/configuring-a-custom-domain-for-your-github-pages-site/managing-a-custom-domain-for-your-github-pages-site)。
