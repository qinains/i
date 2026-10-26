# Optional private model relay

The browser workspace works without a backend when your model API supports CORS.
If it does not, you can operate this private, same-origin relay. It is a separate
`i-relay` executable: no Node.js or Python runtime is needed to run it. Your model
key passes through your own server; do not use an untrusted relay.

GitHub Pages hosts static files only. It cannot execute this service or create a
same-origin API route. Keep the public Pages site in direct mode, or self-host the
static app and relay together behind your own HTTPS origin. There is no shared
public relay operated by this project.

## Build and run (server administrator)

Building from source requires the repository's Rust toolchain. From the root:

```sh
cargo build --locked --release -p i-relay
./target/release/i-relay --help
```

Deploy that executable, then configure the following variables in your service
secret manager. A POSIX shell example, with placeholder origin and API endpoint:

```sh
export HARNESS_RELAY_TOKEN="$(openssl rand -hex 32)"
export HARNESS_PUBLIC_ORIGIN='https://ai.example'
export HARNESS_UPSTREAMS='["https://your-provider.example/v1/chat/completions"]'
export HARNESS_BASE_PATH='/app/'
./target/release/i-relay
```

Do not print or commit the generated relay token. It is separate from the model
API key. The executable accepts no credentials as command arguments and binds
only `127.0.0.1:8080`; use Caddy for public HTTPS. Existing clients keep their own
API URL, model, protocol and key. In Settings explicitly choose **Own same-origin
relay** and enter the relay access token. Direct mode never sends that token.
There is no automatic switch or request replay after an error.

| Variable | Meaning |
| --- | --- |
| `HARNESS_RELAY_TOKEN` | Required, 32-4096 non-whitespace ASCII bytes. Use a random secret. |
| `HARNESS_PUBLIC_ORIGIN` | Exact HTTPS scheme/host/port, without trailing slash or path. HTTP is allowed only for localhost/loopback development. |
| `HARNESS_UPSTREAMS` | JSON array of 1-64 complete HTTPS model endpoints; only port 443, no query, fragment or URL credentials. |
| `HARNESS_BASE_PATH` | Publishing prefix such as `/app/`; defaults to `/`. Keep it intact in proxy routing. |

Chat Completions `/chat/completions`, Responses `/responses` and Anthropic
`/v1/messages` endpoints are accepted only when explicitly allowlisted. Their
native JSON and streamed bytes are not converted. The gateway path is always
`<prefix>api/chat/completions`, independent of the selected protocol.

## Caddy and static files

The relay deliberately serves **no files**. Deploy the generated `dist/site`
output, not a source checkout. See [site deployment](../docs/site-deployment.md).
An example for an app under `/app/`:

```caddyfile
ai.example {
    handle /app/api/chat/completions {
        reverse_proxy 127.0.0.1:8080 {
            flush_interval 1ms
            transport http {
                response_header_timeout 31m
                read_timeout 31m
                write_timeout 31m
            }
        }
    }
    handle {
        root * /srv/lninl/current
        encode zstd gzip
        file_server
    }
}
```

Validate your actual Caddy configuration before reload. Positive `flush_interval`
keeps buffering low without explicitly opting out of disconnect cancellation;
do not use a negative interval to detach upstream work after a user stops.
Do not enable body/header/debug logging, caching, response buffering, retries or
short total stream deadlines on the API route. Restrict access to trusted users.
The reverse proxy must propagate disconnects for cancellation to reach the relay.
See [Caddy streaming and transport documentation](https://caddyserver.com/docs/caddyfile/directives/reverse_proxy).

## Security and limits

- Exact origin, constant-time token-hash comparison and full endpoint allowlist.
  Only POST JSON is accepted. Duplicate routing/auth headers are rejected.
- Every DNS answer must be public. Private, reserved, mapped IPv6 and transition
  ranges fail closed. The chosen address is pinned for the connection, retaining
  the original hostname for TLS validation. System proxies, redirects, automatic
  retries and transparent decompression are disabled.
- Only the matching credential is forwarded: Bearer for Chat/Responses or
  `x-api-key` with fixed `anthropic-version: 2023-06-01` for Messages. Relay tokens,
  cookies, arbitrary headers and browser-only Anthropic flags are never forwarded.
- Request and response bodies are each limited to 48 MiB; request reception has
  a separate 180-second total limit. The model inactivity deadline is 30 minutes,
  renewed by request/response data, with no fixed total stream-duration cap.
- Four admitted requests, 60 admissions per fixed 60-second process-wide window,
  128 HTTP connections and 16 KiB/64 request headers. Multiple replicas need an
  external shared limiter for deployment-wide quotas.
- A disconnect or inactivity expiration cancels upstream work and releases its
  slot. A watchdog also bounds backpressured responses. After headers are sent,
  errors terminate the stream, not fabricate a new status or completed SSE.
- Model keys and bodies exist in memory but are not persisted or logged by the
  relay. This is not protection against a compromised server, OS administrator,
  malicious provider, proxy or extension. Do not publicly expose the token.

Startup errors identify invalid fields without echoing configuration values.
The former Node entrypoint and `HARNESS_STATIC_ROOT` are removed: publish static
artifacts through Caddy instead. API routing and browser configuration remain
unchanged. Ctrl-C stops active connections; failed model requests are not replayed.

## Tests

```sh
cargo test --locked -p i-relay
cargo clippy --locked -p i-relay --all-targets -- -D warnings
npm --prefix web run test:browser -- --grep 'explicit own relay|Anthropic relay|Responses relay'
```

Fixtures use generated fake credentials and loopback mock APIs, never a paid
model. The private-network connector exists only in the ignored `browser_fixture`
test, not in the production binary or any runtime configuration flag. Browser
tests compile that test executable and run the real relay request/stream path.
