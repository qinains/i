# Optional same-origin Harness relay

This is optional. A static HTTPS PWA can call CORS-enabled APIs directly without Node or this service. A PWA does not bypass CORS; desktop users may instead configure their own trusted, narrowly scoped browser extension. This repository does not install extensions or disable browser security.

## Run

Requires Node.js 24+. Build the frontend first from the repository root:

```sh
sh scripts/build_web.sh
export HARNESS_RELAY_TOKEN="$(node -e 'process.stdout.write(require("node:crypto").randomBytes(32).toString("base64url"))')"
export HARNESS_PUBLIC_ORIGIN='https://harness.example'
export HARNESS_UPSTREAMS='["https://your-provider.example/v1/chat/completions"]'
export HARNESS_STATIC_ROOT="$PWD/web"
node server/main.mjs
```

Use your own real deployment origin and endpoint, not the placeholders. Store the relay token in your deployment secret manager; do not commit, print, or put it in URLs. It is separate from the model API key. This service listens only on `127.0.0.1:8080`; put an HTTPS reverse proxy in front. For local-only development the exact public origin may be `http://127.0.0.1:8080`.

In the app explicitly select **Own same-origin relay**, keep the provider API URL/protocol/model/key, and enter the independently generated relay access token. The model key passes through your server; it is not persisted there. Direct mode never sends the relay token. No fallback or automatic retry is performed.

OpenAI-compatible `/chat/completions`, OpenAI `/responses`, and Anthropic `/v1/messages` HTTPS endpoints are supported. Add each complete endpoint to `HARNESS_UPSTREAMS`, for example `https://api.anthropic.com/v1/messages`. The same-origin gateway remains `/api/chat/completions`; it forwards the selected protocol's native JSON/SSE without converting it. Anthropic targets receive `x-api-key` and the fixed `anthropic-version: 2023-06-01`, never Bearer auth or the direct-browser-access header. OpenAI Chat/Responses targets receive Bearer auth, never Anthropic credentials. Arbitrary client headers are not forwarded.

`HARNESS_BASE_PATH` optionally specifies a slash-delimited publishing prefix such as `/harness/`. Keep the prefix intact when forwarding to Node; origin remains the scheme/host/port only. The app computes its relay route relative to its publishing root. The relay can also run without `HARNESS_STATIC_ROOT` if the reverse proxy separately serves the static PWA.

## Reverse proxy

Example Nginx location (configure TLS separately):

```nginx
location / {
    proxy_pass http://127.0.0.1:8080;
    proxy_http_version 1.1;
    proxy_buffering off;
    proxy_request_buffering off;
    proxy_read_timeout 190s;
    client_max_body_size 48m;
    gzip off;
}
```

Disable model-body/header logging at every layer; do not log Authorization, relay tokens or uploaded content. Do not add cache rules to `/api/`. Restrict access to your deployment; a relay token grants access to the configured endpoints and capacity.

## Limits and security

- Exact origin and complete HTTPS endpoint allowlist; only POST JSON, no redirect following, cookies or arbitrary header forwarding.
- Model/relay tokens are never forwarded to the wrong receiver. Relay token comparison is timing-safe for equal-length values.
- DNS results must all be public; the chosen address is pinned into the HTTPS lookup while retaining the original hostname for TLS verification. Private, reserved, mapped IPv6 and transition ranges are rejected conservatively. This may reject some unusual otherwise-routable endpoints.
- Each request/response: 48 MiB; inactivity deadline: 30 minutes, renewed when request/response data arrives (no total stream-duration cap); at most 4 upstream requests; at most 60 accepted requests per minute, shared across this server process. Slow request-body uploads still have Node's separate 180-second request-receive deadline. Multiple replicas need an external shared rate limit if a deployment-wide cap is required.
- A disconnect or deadline aborts upstream; SSE is forwarded incrementally with backpressure. After response headers have been sent, a failure closes the stream instead of forging a new HTTP status. The browser then rejects incomplete streams.
- Static publishing uses generated `web/assets.json`, not a repository directory listing. Unknown paths, dotfiles and symlinks outside the publish root are not served.

Deploy the complete build atomically. Never mix a generated worker/asset list with JS/WASM from another build. First build does not publish anything; you must supply a reachable HTTPS domain/certificate for phone access. On iPhone, localhost refers to the phone itself, not your computer.

## Tests

```sh
npm --prefix server test
npm --prefix web run test:browser
```

Fixtures inject their own DNS/connect functions to use loopback test servers; production CLI has no private-network override. Tests do not read a real API key or call a paid model.
