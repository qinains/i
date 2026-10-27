# Networking, proxies and session recovery

The native core model transport is asynchronous `reqwest` with Rustls and bundled Mozilla trust roots. The optional `plugins/runtime` web worker uses `ureq` 3.4.2 with native platform TLS; its `native-tls` connector feature must remain enabled. The bundled build (`--features bundled`) has one executable: ordinary arguments run the core; `i --runtime ...` runs the optional console/plugins. Workers remain separate bounded processes, but share the same executable file.

TLS certificate and hostname verification remain enabled. For the native-TLS web worker, macOS uses Security.framework, Windows uses Schannel, and Linux uses OpenSSL/system CA certificates (build prerequisites: `pkg-config`, `libssl-dev`; runtime: compatible OpenSSL libraries and CA certificates). Core Rustls and worker platform trust stores can differ. No external curl command or insecure TLS fallback is used. The core does not follow redirects. The native Windows/Linux paths have CI configuration but were not locally executed on this macOS host.

## Configure an Xray proxy

Merge these fields into your API configuration; keep the existing model, endpoint, credential-variable and plugin settings:

```json
{
  "proxy": "socks5h://127.0.0.1:3067",
  "network_retries": 5
}
```

The scheme must match Xray's configured inbound protocol:

| Setting | Behavior |
|---|---|
| `http://127.0.0.1:3067` | HTTP forwarding; CONNECT for HTTPS destinations |
| `socks5://127.0.0.1:3067` | SOCKS5, resolve the destination locally |
| `socks5h://127.0.0.1:3067` | SOCKS5, let the proxy resolve the destination |
| `"proxy": ""` | Force direct connections, overriding proxy environment variables |
| Omit `proxy` / use null | Inherit proxy environment configuration |

Port 3067 alone does not identify the inbound protocol. Xray must be started separately. The application does not modify personal Xray or global proxy configuration.

An alternative for a SOCKS5 inbound is:

```sh
HTTPS_PROXY=socks5h://127.0.0.1:3067 ./scripts/start.sh /path/to/project
```

For an HTTP inbound, replace `socks5h://` with `http://`. Inherited proxy selection follows reqwest's scheme-specific environment configuration, with `ALL_PROXY` as a fallback; `NO_PROXY`/`no_proxy` exclusions apply to inherited proxies. Explicit `proxy` configuration wins and does not inherit exclusions. A valid explicitly selected proxy failing does **not** cause a direct-connection fallback. Unsupported explicit proxy schemes fail before sending a model request. Do not put proxy passwords in committed configuration or paste authenticated proxy URLs into logs/chat; prefer private environment configuration.

API JSON settings configure the agent, not Cargo/git or the whole OS. Environment variables may also affect those development tools. `I_API_CONFIG=/absolute/path/to/config.json ./scripts/start.sh PROJECT` selects another API config for the source launcher; its generated manifest is the standard profile (use the explicit CLI/appropriate manifest for Responses).

Web tools intentionally do not inherit API proxies. They disable environment proxies, resolve with a timeout, validate the actual addresses passed to the connector, and revalidate each redirect. This preserves private/reserved-network protections rather than allowing a proxy to bypass them.

## Reconnect and retry

- `network_retries` is 0..8, default 5. Zero disables retries; the smoke-test examples explicitly use zero.
- Connection/DNS/I/O interruptions and timeouts are retry candidates. HTTP 408, 429, 500, 502, 503, 504 and 529 are also retry candidates.
- Delays start at 1, 2, 4, 8, 16 seconds. Valid `Retry-After` delta-seconds or HTTP dates can increase the delay. If that delay exceeds the remaining budget, stop rather than retry too early.
- Each logical model request has a 180-second network budget including backoff; connection and DNS phases are capped at 15 seconds. Plugin hooks have their own bounded call deadlines and may add wall-clock time.
- Authentication/configuration errors, TLS verification errors, malformed complete JSON, HTTP redirects, oversized bodies, invalid tool batches and plugin failures are not automatically retried.
- The fully prepared model request is reused byte-for-byte. The normal preparation/response hooks are not replayed. Registered `request_retry` observers are notified before another physical attempt; the cache module counts it as an additional preparation. Missing responses remain unknown, never invented hits.
- Local tools/commands and failed worker operations are **not** replayed. Partial model responses are never executed. A remote model request may already have been processed before its response was lost, so retries can incur duplicate billing or repeat provider-side work; this is not an exactly-once API guarantee.
- Ctrl-C in the console aborts the active task. No perpetual background reconnect daemon is installed.

## Resume an existing session

There is no separate `--resume` flag or `/resume` command. Selecting the same session automatically loads its saved history, then the next task continues it:

```sh
i --runtime console -C /path/to/project --session-name=test
# Enter: Continue from the last saved result.
```

`./scripts/start.sh PROJECT` uses the `test` console session. Re-running it for the same workspace and platform data directories resumes that history. If `--session-name` is omitted, the console creates a new generated name; it does not automatically select the most recent session.

The low-level core can instead load an explicit checkpoint:

```sh
i --config /path/to/api.json -C /path/to/project \
  --session /absolute/path/to/session.json 'Continue from the saved results.'
```

Console sessions live under `data/workspaces/<id>/sessions/`. Standalone core `--session-name NAME` uses project `.i/sessions/`, a **different namespace**; do not interchange the two expecting the same file. `i --runtime paths -C PROJECT` shows the platform locations. Existing checkpoints validate the workspace, session version and complete tool-call/result pairing.

If network recovery is exhausted, configured checkpoints retain the user input and completed tool batches. This does not restore an in-flight generation, unsent UI queue, or partial tool batch. A crash/force-kill during tool execution can leave real side effects ahead of the last checkpoint; inspect them before continuing. No cross-process exactly-once guarantee is claimed.

## Offline checks

`tests/network.rs` tests connection refusal followed by restoration, truncated response retry with identical payload and a once-only tool, bounded exhaustion/checkpoints, Retry-After, permanent errors, malformed JSON, proxy routing/local-vs-remote DNS/environment inheritance/direct override, and bundled worker dispatch. `cargo run --locked --manifest-path tools/native-tool/Cargo.toml -- check-tls target/debug/i --web` uses a temporary self-signed local TLS server with a matching SAN. It requires real ClientHello, a certificate/trust error, correct exit/error framing, no HTTP data and no panic/retry for both model and web clients. A socket EOF alone is not certificate rejection: the old test made this mistake and could pass an HTTPS panic. A separate Rust ClientHello regression covers provider dispatch. On non-Windows hosts the packager and package checker run the stricter test against the actual binary. Package checks run an actual console task and its memory/steering workers from the one delivered binary. No paid API requests are required by these checks.
