# Security policy

Do not open public issues containing live credentials, personal conversations,
private workspace files, exploitable production details, or SSH keys.

Use GitHub's private vulnerability reporting when enabled for
https://github.com/qinains/i. If unavailable, open a minimal issue requesting a
private contact channel without including the sensitive details. No response-time
or supported-version guarantee is currently offered.

Provide affected versions, attacker-controlled input, trust boundary, impact,
minimal synthetic reproduction, and expected/observed behavior. Distinguish a
static concern from an executed reproduction. Redact all credentials.

## Boundaries

### Model requests and credentials

The public website is static: it does not provide an application backend that
stores your API key. In direct mode the browser sends the key and model requests
to the API URL you select. A self-hosted Relay receives and forwards these
requests through your server. Verify the destination before saving a connection.

Local storage is not local inference. Task messages, file content used by tools,
images and conversation context may be sent to the selected model service.
That provider's retention and privacy policies apply. Website hosting and model
providers may keep ordinary request logs; this is not a promise of anonymous use.

Remember is optional. Without it, connection credentials stay in page memory;
with it, they persist in this browser's localStorage. Saved credentials are
**not encrypted** by the app. Same-origin scripts, an XSS compromise, or a browser
extension with sufficient permissions may read them. Use a restricted, revocable
API key and do not remember it on shared devices. Never include credentials in
plugin input, workspace files, screenshots, recordings or public reports.

### Browser WASM plugins versus native workers

Browser WASM guest modules execute in dedicated workers. Arbitrary WASM imports
are rejected. The guest ABI exposes no direct DOM, localStorage, network or
connection-configuration interface. File access and per-plugin private storage
are brokered by the host against the installed manifest and current task grant.
Plugins can see their tool input and files allowed by their workspace permission.
Do not pass secrets to a plugin expecting them to be hidden from it.

The default prompt-management plugin has private storage but no workspace-file
permission. AI-requested prompt changes require explicit approval even in
automatic mode, and apply to subsequent tasks. This boundary does not protect
against a compromised host page or privileged browser extension.

Native worker plugins are trusted processes with the user's OS privileges, not
sandboxed WASM guests. The browser disables local shell execution; the native
client can run shell commands. Review plugin source and permissions before use.

### Files and HTML previews

Persistent browser workspaces use local OPFS. Temporary `/tmp` and temporary
workspaces are separate and are not retained after reload. Clearing site data,
browser storage eviction or device loss may delete persistent workspaces too.
Export ZIP backups. Cooperative reader/writer locking is not multi-file rollback,
at-rest encryption, or a substitute for backups.

Preview execution is separate from plugin execution. The iframe does not grant
host same-origin access. Only enable JavaScript for trusted content after review:
preview scripts can read files included in their snapshot. Its isolated storage
is not the app's credential storage. A snapshot is not a full web server, and its
CSP is not a complete network-isolation guarantee. Generated content is untrusted.

### Browser CORS extensions

CORS helper extensions are an opt-in, privileged browser component, distinct from
WASM plugins. Response-header rules do not fix authentication or TLS errors.
Chromium's optional preflight repair uses the debugger permission to synthesize
204 responses only for matching OPTIONS requests in the authorized tab. Real
POST responses, status codes and streamed contents are not replaced.

The debugger can receive authentication metadata for matching requests, even
though the implementation does not log, persist or forward those credentials.
Install only trusted extensions and disable access when no longer needed.
Firefox and Safari packages do not provide the Chromium debugger repair.
PWA installation and `file://` do not bypass CORS.

Before making a repository public, scan both exported files and the exact Git
history to be pushed. Removing a secret from the current tree does not remove it
from history. Rotate any credential discovered in previously shared artifacts.
