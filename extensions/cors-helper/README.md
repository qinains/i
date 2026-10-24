# Harness CORS Helper 0.2.0

## Chrome / Edge (desktop)

1. Unzip the chromium ZIP into a permanent directory.
2. Open chrome://extensions or edge://extensions, enable Developer mode.
3. Choose Load unpacked and select the directory containing manifest.json.
4. Open the Harness page, click this extension's toolbar icon.
5. Save the API settings in the workbench first. The popup automatically reads
   the applied public request URL, not the API key or configuration storage.
   Manual URL entry is collapsed unless detection fails; it accepts a full
   endpoint such as https://api.deepseek.com/chat/completions, never a key.
6. Click Enable, grant the two website permissions, then retry the
   request manually. Keep Harness transport set to Direct.
7. Turn it off in the same popup after use. Restarting the browser disables it.

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

The extension changes response headers only. It does not read, store or forward
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

OPTIONS must already return a successful status. Missing CORS response headers
can be repaired; server-side OPTIONS rejection, redirects, invalid credentials,
quota errors and network/TLS failures cannot. It does not make every API usable
in a browser. Credentials/cookies are not enabled by the helper.

网页不能自动安装扩展。解压后通过浏览器开发者入口安装；扩展默认关闭。
先保存网页 API 设置，再在 Harness 标签页点击扩展；自动读取地址后点击“启用”并授权。
识别不到时才展开手动填写。旧版扩展需更新文件并重新加载，旧版工作台需应用更新。
仅修正 CORS 响应头，OPTIONS 被服务器拒绝时仍会失败，不会伪造成功响应。
Safari 必须通过 Xcode/容器应用启用；Firefox 未签名包只能临时加载。

## Official references

- https://developer.chrome.com/docs/extensions/reference/api/declarativeNetRequest
- https://extensionworkshop.com/documentation/develop/temporary-installation-in-firefox/
- https://developer.apple.com/documentation/safariservices/packaging-a-web-extension-for-safari
- https://developer.apple.com/documentation/safariservices/blocking-content-with-your-safari-web-extension
