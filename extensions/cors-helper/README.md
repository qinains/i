# Harness CORS Helper 0.1.0

## Chrome / Edge (desktop)

1. Unzip the chromium ZIP into a permanent directory.
2. Open chrome://extensions or edge://extensions, enable Developer mode.
3. Choose Load unpacked and select the directory containing manifest.json.
4. Open the Harness page, click this extension's toolbar icon.
5. Copy the full Request URL from Harness Settings > Model connection into the
   extension (for example https://api.deepseek.com/chat/completions).
6. Click Authorize and enable, grant the two website permissions, then retry the
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
stored. No content scripts, remote code or general fetch proxy are included.

OPTIONS must already return a successful status. Missing CORS response headers
can be repaired; server-side OPTIONS rejection, redirects, invalid credentials,
quota errors and network/TLS failures cannot. It does not make every API usable
in a browser. Credentials/cookies are not enabled by the helper.

网页不能自动安装扩展。解压后通过浏览器开发者入口安装；扩展默认关闭。
在 Harness 标签页点击扩展，填写网页设置显示的完整请求 URL，授权后启用。
仅修正 CORS 响应头，OPTIONS 被服务器拒绝时仍会失败，不会伪造成功响应。
Safari 必须通过 Xcode/容器应用启用；Firefox 未签名包只能临时加载。

## Official references

- https://developer.chrome.com/docs/extensions/reference/api/declarativeNetRequest
- https://extensionworkshop.com/documentation/develop/temporary-installation-in-firefox/
- https://developer.apple.com/documentation/safariservices/packaging-a-web-extension-for-safari
- https://developer.apple.com/documentation/safariservices/blocking-content-with-your-safari-web-extension
