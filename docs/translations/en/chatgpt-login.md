# Manage sign-in plugins with i

> **Claude uses Console API billing, not your Pro or Max subscription allowance.**
> Signing in without entering an API key does not make requests free. Check your Console organization, workspace, and billing permissions before authorizing access.

Use `i` to manage connections without memorizing helper executable names or writing connection settings by hand. Native sign-in plugins run with your operating-system user permissions. They are not workspace WASM tool plugins; use the correct installation package.

## Set up from the terminal

Start `i` and enter the local management command, even if you have not configured a model yet:

```text
/plugins
```

Choose ChatGPT or Claude, review the permissions, then install and enable the plugin. Enter `/login`, select a sign-in plugin, and authorize it in your system browser. Pick a model from the list or enter a model ID you can use. On success, `i` saves the connection for your next task. You can also enter the commands directly:

```text
/plugins install chatgpt
/plugins enable chatgpt
/login chatgpt
```

Use `/cancel` to close the menu. You cannot change plugins or connections while a task is running.
Authorization and model listing do not send generation requests. The connection changes only after both sign-in and model selection succeed. Failures and cancellations leave the previous connection intact, and `i` saves a private configuration backup before switching.

## Manage plugins from the command line

### Change the model and reasoning effort

`/model` reads the model catalog from the current native connection plugin. Choose a number from the list or enter a model ID directly. Standard API-key connections do not query a catalog; enter the model ID yourself. This does not sign in again or switch accounts:

```text
/model
/model status
/model gpt-6.1-sol
/model gpt-6.1-sol high
/model effort high
/model effort auto
```

Change settings only when no tasks, approvals, or queued items are pending. Settings are saved per connection, with a private backup before writing. Canceling the model list leaves the configuration unchanged. Switching only the model resets effort to the provider default so you do not carry over another model's settings.
`auto` and `default` leave the effort unspecified. Other values are `none`, `minimal`, `low`, `medium`, `high`, `xhigh`, and `max`. Not all models support every value, and the catalog may not advertise their capabilities.
Changing the setting does not send a billable generation request. The provider validates support on subsequent requests.
Responses uses `reasoning.effort`; Chat Completions uses `reasoning_effort`. Claude uses `output_config.effort` with adaptive thinking. `none` requests that thinking be disabled on models that support that mode. Do not assume every model supports higher effort or disabling thinking.
See [OpenAI reasoning](https://developers.openai.com/api/docs/guides/reasoning) and [Claude effort](https://platform.claude.com/docs/en/build-with-claude/effort).

```sh
i plugins list
i plugins install chatgpt
i plugins enable chatgpt
i login chatgpt
i plugins status chatgpt
i plugins models chatgpt
i plugins logout chatgpt
i plugins disable chatgpt
i plugins update chatgpt
i plugins reconnect chatgpt
i plugins remove chatgpt
```

Switch to an existing account with `i plugins use chatgpt ACCOUNT_ID`. Use `i login chatgpt --account ACCOUNT_ID` to reauthorize an existing registration, or `i plugins logout chatgpt --account ACCOUNT_ID` to sign out of a specific account. Reauthorization stages the registration without copying the old rotating tokens. It does not change the account on the current connection until you confirm the model.

Substitute `claude` for `chatgpt` to manage the Claude Console plugin. Installation and updates require confirmation. After updating, re-enable the plugin and sign in to avoid silently replacing an active connection.
If you update only the helper and want to keep the same account, re-enable the plugin and run `i plugins reconnect chatgpt`. This keeps the current account and model settings and binds the active connection to the verified new executable. It does not sign in again or send a generation request.
While installing, `i` shows version lookup, bytes downloaded, checksum verification, and save progress. Use `/cancel` to stop. The connection times out after 10 seconds, and each download can take up to 120 seconds. Failures do not trigger an automatic retry or enable the plugin.
GitHub downloads use the terminal's proxy environment variables, not your browser's proxy settings. If needed, start with `HTTPS_PROXY=http://127.0.0.1:3067 i`, replacing the address and port with your own proxy settings.

You can also change proxy rules from within `i`. By default, rules are saved per user in `~/.i/config/proxy.json` (under `I_CONFIG_DIR` if set), not in the workspace:

```text
/proxy                         # Show the default and effective per-purpose rules
/proxy 3067                    # Equivalent to http://127.0.0.1:3067
/proxy :3067
/proxy 127.0.0.1:3067
/proxy socks5://127.0.0.1:3067
/proxy no                      # Force a direct connection; noproxy also works
/proxy env                     # Restore environment or model settings
/proxy chatgpt 3067            # ChatGPT sign-in, model listing, and inference only
/proxy claude no               # Connect directly to Claude Console
/proxy plugins no              # Download plugins without a proxy
/proxy model 3067              # Proxy standard model API requests
/proxy chatgpt default         # Remove the ChatGPT override and use the default
```

Rules apply in this order: per-purpose rule, user default, then original environment or configuration. An explicit `no` never falls back to an environment proxy. Changes affect later operations only; they do not interrupt or replay active requests. Configure browsers and already-running standalone services separately.
HTTP, HTTPS, SOCKS5, and SOCKS5h are supported. Local authorization callbacks do not use the proxy. Proxy URLs cannot include usernames, passwords, queries, or paths, so credentials do not end up in command history. This is a built-in host command; it does not expose proxy settings or account credentials to workspace WASM plugins.
Removing an installation selection retains old packages and credentials and does not revoke remote authorization. To sign out, run `i plugins logout NAME` before disabling or removing the plugin. If remote revocation is not confirmed, check the provider's account settings.

The installer downloads the matching connection attachment from the official GitHub Release. It checks the SHA-256 provided by GitHub, the ZIP member allowlist, and the target platform, then verifies the executable digest before each invocation. If no platform matches, it stops rather than running third-party installation scripts. The checksum verifies integrity, not publisher identity. The macOS **connection packages** are not Developer ID signed or notarized. Plugin installation does not change PATH or require administrator privileges.

Developers can test a locally built package with `i plugins install chatgpt --package /absolute/path/package.zip`. After confirmation, the installer still checks archive members and the target platform. The plugin starts disabled, and its executable digest is checked before use.
A local package is labeled `local-build`, not verified as an official release. Install only native code you trust.
A browser callback confirmation does not mean sign-in is complete. Wait for the terminal to finish token exchange, identity verification, and model selection.

Older user-level `desktop.json` files may still list `search` and `find_files`, which are now built into the core. On load, the host drops recognizable old built-in search entries and binds old built-in workers from the same user configuration directory to the current executable, without changing the file on disk. Conflicting names for custom workers and plugins still produce an error; the host does not skip duplicate checks. These errors occur before model requests, so signing in again is unnecessary.

### Diagnose failed generation

Native ChatGPT connections use the Responses API with `store:false` and `stream:true`, not Chat Completions. The terminal prefers SSE, but also accepts a complete `application/json` Responses object if it has `object:response`, a valid ID, `status:completed`, and valid output. Without a `Content-Type` header, it detects SSE or JSON from the body and then applies the same validation. It rejects explicitly marked HTML or other unsupported media types. Failure diagnostics show only the format category and byte count, not the response body.
HTML, Chat Completions objects, invalid JSON, and incomplete tool calls are not accepted. A change in response format does not cause a request to be resent.
The local web service forwards valid SSE live. If the content type is missing or the response is JSON, it validates the complete response before providing valid SSE to the browser. This compatibility path waits for completion and does not automatically resend. A local HTTP 400 in the browser is not necessarily the upstream status: diagnostics distinguish upstream HTTP errors, network failures, and format failures.

On failure, the diagnostic identifies the stage: request preparation, authorization, HTTP, response format, or stream parsing. HTTP diagnostics include only the status, known error codes or parameter names, and a limited request ID. They do not display raw errors, prompts, or tokens. Network or format errors do not clear your sign-in or automatically switch accounts, models, or API-key billing paths.

## Configuration and credentials

- `<config_dir>/native-plugins.json` stores the installation path, digest, and enabled state, not OAuth tokens.
- `<data_dir>/connections/accounts/NAME` stores the plugin's credentials; workspace WASM plugins do not receive them.
- The sign-in menu fills in the model configuration. You do not need to write `chatgpt.json`.
- macOS and Linux use private directories and file permissions. Windows inherits the user directory ACL.
- The application does not encrypt credentials or store them in a system keychain. Malicious native programs running as the same user may still read them.

Do not put these directories in workspaces, shared folders, version control, or public backups. Direct API-key connections continue to work without a sign-in plugin.

## Pair with a browser

After installing and enabling the plugin, run this on the same computer as the browser:

```sh
i plugins serve chatgpt --origin https://i.lninl.com --port 0
```

For Claude, substitute `claude` for `chatgpt`. Keep the terminal running and open the printed pairing link. Confirm the plugin, account, and model on the page, then save. Pairing creates an unsaved draft first and does not automatically replace an existing connection.
Use a new link after restarting the service. Do not share pairing links: each contains a local capability secret.

The service listens only on `127.0.0.1`, is bound to the specified page origin, and does not proxy arbitrary URLs. HTTPS pages accessing loopback may need local-network permission. Pairing does not bypass browser permissions or CORS. A phone cannot run the native plugin on its own, and deleting a browser connection does not sign out of the native account.

## ChatGPT limitations and troubleshooting

The plugin uses official OAuth dynamic client registration, PKCE, a separate host identity, and separate credential storage. It does not read your personal `~/.codex/auth.json` or borrow proprietary client IDs. See the [official sign-in documentation](https://developers.openai.com/siwc/token-sharing-open-source/sign-in) and [profiles and sessions](https://developers.openai.com/siwc/token-sharing-open-source/profiles-and-sessions).

Inference uses the public Responses API, not the private `backend-api`. OpenAI determines eligibility, workspace policy, and authorization limits; signing in does not guarantee inference access. Subscription-based authorization uses fixed settings of `store: false` and `stream: true`. The current preview ignores the maximum output token setting; see [preview limitations](https://developers.openai.com/siwc/token-sharing-open-source/preview-limitations).

Failures distinguish authorization-code exchange, session refresh, and network errors. Diagnostics show HTTP status codes without displaying raw token endpoint responses, credentials, or authorization codes. Failed authorization or model requests are not automatically replayed. If a code exchange is rejected, start sign-in again and check the version, system clock, network, and proxy settings. The status code alone does not tell you why it was rejected.

## Claude Console limitations

Follow the [official installation guide](https://platform.claude.com/docs/en/cli-sdks-libraries/cli/quickstart) to install `ant` and ensure it is on PATH. The connection package neither bundles `ant` nor runs its installation script for you. Then use `i login claude`.

The plugin uses an isolated HOME, `ANTHROPIC_CONFIG_DIR`, and its own profile. It does not read existing Claude Code or personal ant profiles. It currently supports one Console profile. Reauthorization can change the workspace, and subsequent requests may use the new workspace; start a new session if needed. **This does not use a Pro or Max subscription allowance.**

## Verification scope

Automated tests use isolated directories and local OAuth/TLS fixtures. They do not use real personal tokens, authorize access on your behalf, or send billable requests. Passing offline tests does not prove that a provider will grant access to a real account. Helpers retain their internal worker and service protocols; user-facing operations go through `i`.
