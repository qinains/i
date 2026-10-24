# Harness CORS Helper 0.3.1

## 预检修复（仅 Chrome / Edge）

界面只有一个按钮：点击“启动”并确认，运行后同一按钮变为“停止”。Chrome / Edge
启动时一并开启响应头规则和预检修复，任一步失败会回滚。仅对当前标签页、当前 origin 与指定 API 完整 URL 的
POST 预检合成 204，真实 POST 的鉴权、状态、正文、回复及 SSE 保持原样。
不会自动重试请求，也不会自动保存/恢复调试授权。关闭修复、关闭浏览器调试提示、
关闭标签页或跳转到其他 origin 都会停用；更换 API 后先停止，再重新启动。

Chromium 包要求 Chrome / Edge 118+，安装时额外声明 `debugger` 权限（Chrome 不允许
将此权限设为 optional）。浏览器会显示调试横幅；其他调试器或 DevTools 可能与之冲突。
该权限能力强于普通 CORS 规则，不是“只能访问预检”的浏览器权限沙箱。
实现仅对精确 URL 开启 Fetch 拦截，并按 OPTIONS/Origin/POST 过滤；普通请求立即继续。
调试协议可能把匹配请求的认证头等元数据交给后台，但实现不记录、存储或转发这些数据。
Firefox/Safari 包不声明 debugger，也不提供此模式，不应把 Chromium 测试当成跨浏览器验证。

## Chrome / Edge (desktop)

1. Unzip the chromium ZIP into a permanent directory.
2. Open chrome://extensions or edge://extensions, enable Developer mode.
3. Choose Load unpacked and select the built directory containing manifest.json
   and icon-48.png. Do not load the repository's extensions/cors-helper source
   directory directly. Run python3 scripts/build_cors_extension.py first and load
   dist/cors-helper/chromium, or unzip the published browser ZIP.
4. Open the Harness page, click this extension's toolbar icon.
5. Save the API settings in the workbench first. The popup automatically reads
   the applied public request URL, not the API key or configuration storage.
   Manual URL entry is collapsed unless detection fails; it accepts a full
   endpoint such as https://api.deepseek.com/chat/completions, never a key.
6. Click Start (启动), confirm the debugger notice and website permissions, then
   retry the request manually. Keep Harness transport set to Direct.
7. Click the same button, now Stop (停止), to turn both modes off. Restarting
   the browser disables it. Firefox/Safari use the same button for ordinary mode only.

## Firefox (desktop)

Open about:debugging > This Firefox > Load Temporary Add-on. Select the Firefox
ZIP or its unpacked manifest.json, then follow steps 4-7 above. This unsigned
development package lasts until Firefox restarts. Permanent distribution needs
Mozilla signing; renaming ZIP to XPI does not remove that requirement.

## Safari (macOS development)

Safari cannot directly install a browser-extension ZIP. With Xcode installed:

    xcrun safari-web-extension-converter /path/to/unpacked/safari \
      --project-location /path/to/new-project --app-name "Harness CORS Helper" \
      --bundle-identifier local.lninl.Harness-CORS-Helper --macos-only \
      --copy-resources --no-open --no-prompt

Open the generated Xcode project, build/run the container app, and enable the
extension in Safari Settings > Extensions. For unsigned local development,
enable Safari's developer features and Allow Unsigned Extensions; this may reset
after Safari quits. Distribution requires Apple's app/signing workflow. iPhone
Safari requires an appropriately signed containing app; iPhone Chrome/Edge do
not install these desktop ZIPs.

Safari builds must successfully accept the tab-scoped session rule. If permission
or API support is missing, the popup reports an error and does not broaden rules
to every tab/site. Xcode compilation is not proof of Safari runtime CORS support.

## Scope and limitations

Default OFF. Session-only rules target one tab and the exact API request URL.
Access-Control-Allow-Origin is fixed to the selected page's complete origin,
including its port. HTTP is permitted only for localhost testing. HTTPS is
required for remote sites. No file:// or opaque origins are supported.

Ordinary mode changes response headers only. It does not read, store or forward
API keys, request bodies or model replies; it does not change real HTTP status,
cookies, Origin, Authorization or TLS validation. Only API URL preferences are
stored along with the page origin. A one-off script uses scripting + activeTab
to read only data-model-api-url and data-model-transport from the page root.
No form fields or localStorage/IndexedDB are read. There are no persistent
content scripts, remote code or general fetch proxy. The scripting permission
is inherently powerful; these are audited code restrictions, not a claim that
the browser API itself cannot access page data. Relay mode does not need it.

To update an unpacked 0.1.0 installation, replace its files and reload it in the
extension manager, or remove the old version before loading the new directory.
Update the workbench PWA too if automatic detection is unavailable.

In ordinary mode, OPTIONS must already return a successful status. Missing CORS response headers
can be repaired; server-side OPTIONS rejection, redirects, invalid credentials,
quota errors and network/TLS failures cannot. The explicitly confirmed Chromium
preflight repair described above can synthesize OPTIONS 204, but cannot fix actual
POST authentication, quota or TLS failures. It does not make every API usable
in a browser. Credentials/cookies are not enabled by the helper.

网页不能自动安装扩展。解压后通过浏览器开发者入口安装；扩展默认关闭。
先保存网页 API 设置，再在 Harness 标签页点击扩展；自动读取地址后点击“启动”并授权。
识别不到时才展开手动填写。旧版扩展需更新文件并重新加载，旧版工作台需应用更新。
普通模式仅修正 CORS 响应头，OPTIONS 被服务器拒绝时仍会失败；Chrome / Edge
启动时确认开启上述预检修复，但不会改变真实 POST 的状态和鉴权结果。
Safari 必须通过 Xcode/容器应用启用；Firefox 未签名包只能临时加载。

## Official references

- https://developer.chrome.com/docs/extensions/reference/api/declarativeNetRequest
- https://extensionworkshop.com/documentation/develop/temporary-installation-in-firefox/
- https://developer.apple.com/documentation/safariservices/packaging-a-web-extension-for-safari
- https://developer.apple.com/documentation/safariservices/blocking-content-with-your-safari-web-extension
