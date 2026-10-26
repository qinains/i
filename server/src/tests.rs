use super::*;
use std::net::IpAddr;

const TOKEN: &str = "fixture_relay_token_not_a_real_secret_0123456789";
const ORIGIN: &str = "https://harness.example";
const TARGET: &str = "https://api.example/v1/chat/completions";

fn config() -> Config {
    Config::new(TOKEN, ORIGIN, &[TARGET.to_owned()], "/").unwrap()
}

#[test]
fn config_requires_exact_origin_strong_token_https_targets_and_safe_prefix() {
    for (token, origin, target, prefix) in [
        ("short", ORIGIN, TARGET, "/"),
        (TOKEN, "*", TARGET, "/"),
        (TOKEN, "https://harness.example/", TARGET, "/"),
        (TOKEN, "http://harness.example", TARGET, "/"),
        (TOKEN, ORIGIN, "http://api.example/v1/chat/completions", "/"),
        (TOKEN, ORIGIN, "https://api.example:8443/v1/responses", "/"),
        (TOKEN, ORIGIN, "https://user:secret@api.example/v1/messages", "/"),
        (TOKEN, ORIGIN, "https://api.example/v1/responses?q=1", "/"),
        (TOKEN, ORIGIN, TARGET, "/../"),
    ] {
        assert!(Config::new(token, origin, &[target.to_owned()], prefix).is_err());
    }
    assert!(Config::new(TOKEN, ORIGIN, &[], "/").is_err());
    assert!(Config::new(TOKEN, "http://127.0.0.1:8080", &[TARGET.to_owned()], "/app/").is_ok());
    assert!(config().authorized(TOKEN.as_bytes()));
    assert!(!config().authorized(b"not the relay token"));
}

#[test]
fn address_policy_rejects_special_mapped_and_transition_ranges() {
    for address in [
        "127.0.0.1",
        "10.1.2.3",
        "100.64.0.1",
        "169.254.0.1",
        "172.16.0.1",
        "192.168.1.1",
        "192.0.0.1",
        "198.18.0.1",
        "198.51.100.1",
        "203.0.113.1",
        "224.0.0.1",
        "0.0.0.0",
        "::1",
        "fe80::1",
        "fc00::1",
        "::ffff:127.0.0.1",
        "::ffff:8.8.8.8",
        "2001:db8::1",
        "2002:7f00:1::",
        "2001::1",
        "3fff::1",
    ] {
        assert!(!config::is_public(address.parse::<IpAddr>().unwrap()), "{address}");
    }
    for address in ["8.8.8.8", "1.1.1.1", "2606:4700:4700::1111"] {
        assert!(config::is_public(address.parse().unwrap()), "{address}");
    }
}

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use transport::{Upstream, UpstreamRequest};

fn response(status: u16, data: &'static str) -> Upstream {
    Upstream {
        status: StatusCode::from_u16(status).unwrap(),
        content_type: HeaderValue::from_static("application/json"),
        length: None,
        chunks: Box::pin(stream::iter([Ok(Bytes::from_static(data.as_bytes()))])),
    }
}

fn relay(config: Config) -> Relay {
    let mut relay = Relay::new(config);
    relay.resolve = Arc::new(|_| Box::pin(async { Ok(vec!["8.8.8.8".parse().unwrap()]) }));
    relay.connect = Arc::new(|_| Box::pin(async { Ok(response(200, "{}")) }));
    relay
}

struct Fixture {
    url: String,
    relay: Arc<Relay>,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn fixture(relay: Relay) -> Fixture {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/api/chat/completions", listener.local_addr().unwrap());
    let relay = Arc::new(relay);
    let state = relay.clone();
    let task = tokio::spawn(async move {
        serve(listener, state, std::future::pending()).await.unwrap();
    });
    Fixture { url, relay, task }
}

fn request(f: &Fixture) -> reqwest::RequestBuilder {
    client()
        .post(&f.url)
        .header("origin", ORIGIN)
        .header("content-type", "application/json")
        .header("x-harness-relay-token", TOKEN)
        .header("x-harness-upstream", TARGET)
        .header("authorization", "Bearer FAKE_MODEL_KEY")
        .body("{}")
}

fn client() -> reqwest::Client {
    reqwest::Client::builder().no_proxy().timeout(Duration::from_secs(5)).build().unwrap()
}

fn overwrite(request: reqwest::RequestBuilder, name: &str, value: &str) -> reqwest::RequestBuilder {
    let mut headers = HeaderMap::new();
    headers
        .insert(hyper::header::HeaderName::from_bytes(name.as_bytes()).unwrap(), HeaderValue::from_str(value).unwrap());
    request.headers(headers)
}

async fn wait_for(condition: impl Fn() -> bool) {
    tokio::time::timeout(Duration::from_secs(3), async {
        while !condition() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn authorization_route_media_type_and_body_fail_before_upstream() {
    let called = Arc::new(AtomicUsize::new(0));
    let mut state = relay(config());
    state.connect = {
        let called = called.clone();
        Arc::new(move |_| {
            called.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { Ok(response(200, "{}")) })
        })
    };
    let f = fixture(state).await;
    for (header, value, status) in [
        ("origin", "https://evil.example", 403),
        ("x-harness-relay-token", "wrong", 401),
        ("x-harness-upstream", "https://api.example/v1/responses", 403),
        ("content-type", "text/plain", 415),
    ] {
        let result = overwrite(request(&f), header, value).send().await.unwrap();
        assert_eq!(result.status().as_u16(), status);
        assert!(!result.text().await.unwrap().contains("FAKE_MODEL_KEY"));
    }
    assert_eq!(request(&f).body("invalid-json").send().await.unwrap().status(), 400);
    assert_eq!(client().get(&f.url).send().await.unwrap().status(), 405);
    assert_eq!(client().get(f.url.replace("/api/chat/completions", "/.env")).send().await.unwrap().status(), 404);
    assert_eq!(request(&f).header("origin", ORIGIN).send().await.unwrap().status(), 400);
    assert_eq!(request(&f).query(&[("target", "evil")]).send().await.unwrap().status(), 404);
    assert_eq!(called.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn protocol_credentials_body_pinned_address_and_real_status_are_preserved() {
    let endpoints = [
        TARGET.to_owned(),
        "https://api.example/v1/messages".to_owned(),
        "https://api.example/v1/responses".to_owned(),
    ];
    let observed = Arc::new(Mutex::new(Vec::<UpstreamRequest>::new()));
    let mut state = relay(Config::new(TOKEN, ORIGIN, &endpoints, "/").unwrap());
    state.connect = {
        let observed = observed.clone();
        Arc::new(move |input| {
            observed.lock().unwrap().push(input);
            Box::pin(async { Ok(response(401, "{\"error\":\"fixture\"}")) })
        })
    };
    let f = fixture(state).await;
    for endpoint in &endpoints {
        let input = "{\"model\":\"fixture\", \"stream\":false}";
        let result = overwrite(request(&f), "x-harness-upstream", endpoint)
            .header("x-api-key", "FAKE_ANTHROPIC_KEY")
            .header("anthropic-version", "untrusted")
            .header("cookie", "PRIVATE_FIXTURE")
            .header("x-custom", "private")
            .header("anthropic-dangerous-direct-browser-access", "true")
            .body(input)
            .send()
            .await
            .unwrap();
        assert_eq!(result.status(), 401);
        assert_eq!(result.text().await.unwrap(), "{\"error\":\"fixture\"}");
        let observed = observed.lock().unwrap();
        let sent = observed.last().unwrap();
        assert_eq!(sent.pinned, "8.8.8.8:443".parse().unwrap());
        assert_eq!(sent.url.host_str(), Some("api.example"));
        assert_eq!(sent.body, input);
        assert_eq!(sent.headers["accept-encoding"], "identity");
        for header in
            ["cookie", "origin", "x-harness-relay-token", "x-custom", "anthropic-dangerous-direct-browser-access"]
        {
            assert!(!sent.headers.contains_key(header));
        }
        if endpoint.ends_with("/messages") {
            assert!(!sent.headers.contains_key("authorization"));
            assert_eq!(sent.headers["x-api-key"], "FAKE_ANTHROPIC_KEY");
            assert_eq!(sent.headers["anthropic-version"], "2023-06-01");
        } else {
            assert_eq!(sent.headers["authorization"], "Bearer FAKE_MODEL_KEY");
            assert!(!sent.headers.contains_key("x-api-key"));
            assert!(!sent.headers.contains_key("anthropic-version"));
        }
    }
}

#[tokio::test]
async fn mixed_private_dns_redirects_and_declared_oversize_fail_closed() {
    let called = Arc::new(AtomicBool::new(false));
    let mut state = relay(config());
    state.resolve =
        Arc::new(|_| Box::pin(async { Ok(vec!["8.8.8.8".parse().unwrap(), "127.0.0.1".parse().unwrap()]) }));
    state.connect = {
        let called = called.clone();
        Arc::new(move |_| {
            called.store(true, Ordering::SeqCst);
            Box::pin(async { Ok(response(200, "{}")) })
        })
    };
    let f = fixture(state).await;
    assert_eq!(request(&f).send().await.unwrap().status(), 502);
    assert!(!called.load(Ordering::SeqCst));
    for status in [301, 302, 307, 308, 200] {
        let mut state = relay(config());
        state.connect = Arc::new(move |_| {
            Box::pin(async move {
                let mut result = response(status, "{}");
                if status == 200 {
                    result.length = Some(49 * 1024 * 1024);
                }
                Ok(result)
            })
        });
        let f = fixture(state).await;
        assert_eq!(request(&f).send().await.unwrap().status(), 502);
    }
}

#[tokio::test]
async fn request_byte_rate_and_concurrency_limits_apply() {
    let mut settings = config();
    settings.limits.bytes = 4;
    settings.limits.rate = 1;
    let f = fixture(relay(settings)).await;
    assert_eq!(request(&f).body("12345").send().await.unwrap().status(), 413);
    assert_eq!(request(&f).send().await.unwrap().status(), 429);
    let mut settings = config();
    settings.limits.concurrency = 1;
    settings.limits.inactivity = Duration::from_millis(100);
    let mut state = relay(settings);
    state.resolve = Arc::new(|_| Box::pin(std::future::pending()));
    let f = fixture(state).await;
    let pending = tokio::spawn(request(&f).send());
    wait_for(|| f.relay.slots.available_permits() == 0).await;
    assert_eq!(request(&f).send().await.unwrap().status(), 429);
    assert_eq!(pending.await.unwrap().unwrap().status(), 504);
    wait_for(|| f.relay.slots.available_permits() == 1).await;
}

#[tokio::test]
async fn active_sse_lives_beyond_one_inactivity_period_and_bytes_are_unmodified() {
    let mut settings = config();
    settings.limits.inactivity = Duration::from_millis(200);
    let mut state = relay(settings);
    state.connect = Arc::new(|_| {
        Box::pin(async {
            let chunks = stream::unfold(0, |index| async move {
                if index == 9 {
                    return None;
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
                Some((Ok(Bytes::from_static(b"data: token\n\n")), index + 1))
            });
            Ok(Upstream {
                status: StatusCode::OK,
                content_type: HeaderValue::from_static("text/event-stream"),
                length: None,
                chunks: Box::pin(chunks),
            })
        })
    });
    let f = fixture(state).await;
    let result = request(&f).send().await.unwrap();
    assert_eq!(result.headers()["content-type"], "text/event-stream");
    assert_eq!(result.headers()["x-accel-buffering"], "no");
    assert_eq!(result.headers()["cache-control"], "no-store");
    assert_eq!(result.text().await.unwrap(), "data: token\n\n".repeat(9));
}

struct OnDrop(Arc<AtomicBool>);
impl Drop for OnDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

#[tokio::test]
async fn disconnect_before_headers_cancels_pending_upstream_and_releases_slot() {
    let started = Arc::new(AtomicBool::new(false));
    let dropped = Arc::new(AtomicBool::new(false));
    let mut state = relay(config());
    state.connect = {
        let started = started.clone();
        let dropped = dropped.clone();
        Arc::new(move |_| {
            let started = started.clone();
            let dropped = dropped.clone();
            Box::pin(async move {
                let _guard = OnDrop(dropped);
                started.store(true, Ordering::SeqCst);
                std::future::pending().await
            })
        })
    };
    let f = fixture(state).await;
    let pending = tokio::spawn(request(&f).send());
    wait_for(|| started.load(Ordering::SeqCst)).await;
    pending.abort();
    let _ = pending.await;
    wait_for(|| dropped.load(Ordering::SeqCst)).await;
    wait_for(|| f.relay.slots.available_permits() == 4).await;
}

#[tokio::test]
async fn response_limit_and_stall_close_stream_without_fake_success() {
    for stall in [false, true] {
        let mut settings = config();
        settings.limits.bytes = 8;
        settings.limits.inactivity = Duration::from_millis(100);
        let mut state = relay(settings);
        state.connect = Arc::new(move |_| {
            Box::pin(async move {
                let chunks =
                    stream::once(async { Ok(Bytes::from_static(b"partial")) }).chain(stream::once(async move {
                        if stall {
                            std::future::pending::<()>().await;
                        }
                        Ok(Bytes::from_static(b"too many bytes"))
                    }));
                Ok(Upstream {
                    status: StatusCode::OK,
                    content_type: HeaderValue::from_static("text/event-stream"),
                    length: None,
                    chunks: Box::pin(chunks),
                })
            })
        });
        let f = fixture(state).await;
        if let Ok(result) = request(&f).send().await {
            assert!(result.text().await.is_err());
        }
        wait_for(|| f.relay.slots.available_permits() == 4).await;
    }
}

#[tokio::test]
async fn slow_request_body_has_a_separate_receive_deadline() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut settings = config();
    settings.limits.body_timeout = Duration::from_millis(100);
    let f = fixture(relay(settings)).await;
    let address = url::Url::parse(&f.url).unwrap();
    let mut socket = tokio::net::TcpStream::connect(("127.0.0.1", address.port().unwrap())).await.unwrap();
    socket.write_all(format!("POST /api/chat/completions HTTP/1.1\r\nHost: localhost\r\nOrigin: {ORIGIN}\r\nX-Harness-Relay-Token: {TOKEN}\r\nX-Harness-Upstream: {TARGET}\r\nContent-Type: application/json\r\nContent-Length: 100\r\n\r\n{{").as_bytes()).await.unwrap();
    let mut buffer = [0; 1024];
    let received = tokio::time::timeout(Duration::from_secs(3), socket.read(&mut buffer)).await.unwrap().unwrap();
    assert!(String::from_utf8_lossy(&buffer[..received]).starts_with("HTTP/1.1 408"));
    wait_for(|| f.relay.slots.available_permits() == 4).await;
}

#[tokio::test]
async fn dropping_response_cancels_stream_and_releases_admission() {
    let dropped = Arc::new(AtomicBool::new(false));
    let mut state = relay(config());
    state.connect = {
        let dropped = dropped.clone();
        Arc::new(move |_| {
            let dropped = dropped.clone();
            Box::pin(async move {
                let chunks = stream::unfold(OnDrop(dropped), |guard| async move {
                    tokio::time::sleep(Duration::from_millis(20)).await;
                    Some((Ok(Bytes::from_static(b"data: token\n\n")), guard))
                });
                Ok(Upstream {
                    status: StatusCode::OK,
                    content_type: HeaderValue::from_static("text/event-stream"),
                    length: None,
                    chunks: Box::pin(chunks),
                })
            })
        })
    };
    let f = fixture(state).await;
    let result = request(&f).send().await.unwrap();
    assert_eq!(f.relay.slots.available_permits(), 3);
    drop(result);
    wait_for(|| dropped.load(Ordering::SeqCst)).await;
    wait_for(|| f.relay.slots.available_permits() == 4).await;
}

#[tokio::test]
async fn watchdog_releases_a_backpressured_unread_response() {
    let mut settings = config();
    settings.limits.concurrency = 1;
    settings.limits.inactivity = Duration::from_millis(200);
    let mut state = relay(settings);
    state.connect = Arc::new(|_| {
        Box::pin(async {
            static DATA: [u8; 65536] = [b'x'; 65536];
            let chunks = stream::repeat_with(|| Ok(Bytes::from_static(&DATA)));
            Ok(Upstream {
                status: StatusCode::OK,
                content_type: HeaderValue::from_static("text/event-stream"),
                length: None,
                chunks: Box::pin(chunks),
            })
        })
    });
    let f = fixture(state).await;
    let result = request(&f).send().await.unwrap();
    wait_for(|| f.relay.slots.available_permits() == 1).await;
    drop(result);
}

#[tokio::test]
async fn publishing_prefix_is_exact_and_static_files_are_never_served() {
    let state = relay(Config::new(TOKEN, ORIGIN, &[TARGET.to_owned()], "/app/").unwrap());
    let mut f = fixture(state).await;
    assert_eq!(request(&f).send().await.unwrap().status(), 404);
    f.url = f.url.replace("/api/", "/app/api/");
    assert_eq!(request(&f).send().await.unwrap().status(), 200);
    for path in ["/", "/app/", "/app/assets.json", "/app/../.env", "/app/api/chat/completions?target=bad"] {
        let mut url = url::Url::parse(&f.url).unwrap();
        url.set_path(path);
        url.set_query(None);
        assert_eq!(client().get(url).send().await.unwrap().status(), 404);
    }
}

// Only this test executable can connect to the fixed loopback mock API.
#[tokio::test(flavor = "current_thread")]
#[ignore = "Long-running browser test fixture; never included in the production CLI"]
async fn browser_fixture() {
    let origin = std::env::var("HARNESS_TEST_ORIGIN").unwrap();
    let parsed = url::Url::parse(&origin).unwrap();
    assert_eq!(parsed.scheme(), "http");
    assert_eq!(parsed.host_str(), Some("127.0.0.1"));
    assert!(parsed.port().is_some());
    let endpoints = [
        "https://api.example/v1/chat/completions",
        "https://api.example/v1/messages",
        "https://api.example/v1/responses",
    ]
    .map(str::to_owned);
    let mut state = relay(Config::new(&"fixture-relay-token-".repeat(3), &origin, &endpoints, "/").unwrap());
    state.connect = Arc::new(|input| {
        Box::pin(async move {
            let result = reqwest::Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .read_timeout(Duration::from_secs(30))
                .build()
                .map_err(|_| ())?
                .post(format!("http://127.0.0.1:4174{}", input.url.path()))
                .headers(input.headers)
                .body(input.body)
                .send()
                .await
                .map_err(|_| ())?;
            let status = result.status();
            let content_type =
                result.headers().get("content-type").cloned().unwrap_or(HeaderValue::from_static("application/json"));
            let length = result.content_length();
            let chunks = stream::try_unfold(result, |mut result| async move {
                Ok::<_, ()>(result.chunk().await.map_err(|_| ())?.map(|chunk| (chunk, result)))
            });
            Ok(Upstream { status, content_type, length, chunks: Box::pin(chunks) })
        })
    });
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    println!("HARNESS_RELAY_FIXTURE {{\"port\":{}}}", listener.local_addr().unwrap().port());
    serve(listener, Arc::new(state), std::future::pending()).await.unwrap();
}
