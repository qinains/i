use super::*;
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::{body::Incoming, server::conn::http1, service::service_fn, Request, Response};
use hyper_util::rt::TokioIo;
use jsonwebtoken::{encode, EncodingKey, Header};
use serde_json::{json, Value};
use std::{
    convert::Infallible,
    io::Write,
    path::Path,
    process::{Command, Stdio},
    sync::{Arc, Mutex},
};
use tokio_rustls::{rustls, TlsAcceptor};

#[derive(Default)]
struct Exchange {
    nonce: String,
    challenge: String,
    requests: Vec<(String, String, String)>,
    refreshes: usize,
    reject_refresh: bool,
    reject_code: bool,
    wrong_subject: bool,
    inference_status: u16,
    json_response: bool,
    inference_mime: Option<&'static str>,
    omit_inference_content_type: bool,
    inference_body: Option<&'static str>,
}
struct Fixture {
    http: reqwest::Client,
    exchange: Arc<Mutex<Exchange>>,
    task: tokio::task::JoinHandle<()>,
    _keys: tempfile::TempDir,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}
fn openssl_program() -> PathBuf {
    // Do not resolve `bash` on Windows: it may be WSL rather than Git Bash.
    #[cfg(windows)]
    for root in ["ProgramFiles", "ProgramFiles(x86)"] {
        if let Some(root) = std::env::var_os(root) {
            for relative in ["Git/usr/bin/openssl.exe", "Git/mingw64/bin/openssl.exe"] {
                let candidate = PathBuf::from(&root).join(relative);
                if candidate.is_file() {
                    return candidate;
                }
            }
        }
    }
    PathBuf::from("openssl")
}

fn openssl_command(directory: &Path) -> Command {
    let mut command = Command::new(openssl_program());
    command
        .current_dir(directory)
        .env("HOME", directory)
        .env("USERPROFILE", directory)
        // OpenSSL documents the empty string as disabling configuration loading.
        .env("OPENSSL_CONF", "")
        .env_remove("OPENSSL_CONF_INCLUDE")
        .env_remove("OPENSSL_MODULES")
        .env_remove("OPENSSL_ENGINES")
        .env_remove("RANDFILE");
    command
}

fn openssl(directory: &Path, args: &[&str], input: &[u8]) -> Vec<u8> {
    let mut child = openssl_command(directory)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Native OpenSSL is required for ephemeral OAuth test keys");
    child.stdin.take().unwrap().write_all(input).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "ephemeral OpenSSL {} failed ({}): {}",
        args[0],
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

fn ephemeral_signing_keys(directory: &Path) -> (EncodingKey, Value) {
    let pem = openssl(directory, &["genrsa", "2048"], b"");
    let signing = EncodingKey::from_rsa_pem(&pem).unwrap();
    let modulus = String::from_utf8(openssl(directory, &["rsa", "-noout", "-modulus"], &pem)).unwrap();
    let hex = modulus.trim().strip_prefix("Modulus=").unwrap();
    let modulus: Vec<u8> = (0..hex.len()).step_by(2).map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap()).collect();
    let jwks = json!({"keys":[{"kty":"RSA","kid":"ephemeral","alg":"RS256","use":"sig","n":URL_SAFE_NO_PAD.encode(modulus),"e":"AQAB"}]});
    (signing, jwks)
}

#[test]
fn ephemeral_key_generation_is_native_path_safe_and_verifiable() {
    let directory = tempfile::Builder::new().prefix("oauth key \u{6d4b}\u{8bd5} ").tempdir().unwrap();
    let command = openssl_command(directory.path());
    let program = Path::new(command.get_program()).file_stem().unwrap();
    assert_eq!(program, "openssl", "ephemeral key generation must not launch a shell");
    assert!(command.get_envs().any(|(key, value)| key == "OPENSSL_CONF" && value == Some(std::ffi::OsStr::new(""))));
    let (first, jwks) = ephemeral_signing_keys(directory.path());
    let (_, second_jwks) = ephemeral_signing_keys(directory.path());
    assert_ne!(jwks["keys"][0]["n"], second_jwks["keys"][0]["n"], "each fixture needs fresh RSA entropy");
    assert_eq!(jwks["keys"][0]["e"], "AQAB");
    let modulus = URL_SAFE_NO_PAD.decode(jwks["keys"][0]["n"].as_str().unwrap()).unwrap();
    assert_eq!(modulus.len(), 256, "lifecycle tests require RSA-2048");
    let jwks: JwkSet = serde_json::from_value(jwks).unwrap();
    let token = encode(&Header::new(Algorithm::RS256), &json!({"exp":now()+60,"fixture":true}), &first).unwrap();
    let mut validation = Validation::new(Algorithm::RS256);
    validation.validate_aud = false;
    let claims = decode::<Value>(&token, &DecodingKey::from_jwk(&jwks.keys[0]).unwrap(), &validation).unwrap();
    assert_eq!(claims.claims["fixture"], true);
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 0, "key generation must not need any file paths");
}
impl Fixture {
    async fn start() -> Self {
        let keys = tempfile::tempdir().unwrap();
        let (signing, jwks) = ephemeral_signing_keys(keys.path());
        let cert = rcgen::generate_simple_self_signed(vec!["auth.openai.com".into(), "api.openai.com".into()]).unwrap();
        let tls = rustls::ServerConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_no_client_auth()
            .with_single_cert(
                vec![cert.cert.der().clone()],
                rustls::pki_types::PrivatePkcs8KeyDer::from(cert.signing_key.serialize_der()).into(),
            )
            .unwrap();
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let address = listener.local_addr().unwrap();
        let http = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .add_root_certificate(reqwest::Certificate::from_der(cert.cert.der()).unwrap())
            .resolve("auth.openai.com", address)
            .resolve("api.openai.com", address)
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap();
        let exchange = Arc::new(Mutex::new(Exchange::default()));
        let shared = exchange.clone();
        let task = tokio::spawn(async move {
            let acceptor = TlsAcceptor::from(Arc::new(tls));
            loop {
                let (socket, _) = listener.accept().await.unwrap();
                let acceptor = acceptor.clone();
                let shared = shared.clone();
                let signing = signing.clone();
                let jwks = jwks.clone();
                tokio::spawn(async move {
                    let Ok(socket) = acceptor.accept(socket).await else {
                        return;
                    };
                    let service = service_fn(move |request: Request<Incoming>| {
                        let shared = shared.clone();
                        let signing = signing.clone();
                        let jwks = jwks.clone();
                        async move {
                            let path = request.uri().path().to_owned();
                            let auth = request
                                .headers()
                                .get("authorization")
                                .map(|v| v.to_str().unwrap().to_owned())
                                .unwrap_or_default();
                            let body =
                                String::from_utf8(request.into_body().collect().await.unwrap().to_bytes().to_vec())
                                    .unwrap();
                            let mut state = shared.lock().unwrap();
                            state.requests.push((path.clone(), auth.clone(), body.clone()));
                            let mut status = 200;
                            let mut content_type = "application/json";
                            let mut output = match path.as_str() {
                                "/.well-known/jwks.json" => jwks.to_string(),
                                "/api/accounts/oauth/token" => {
                                    let form: HashMap<String, String> =
                                        url::form_urlencoded::parse(body.as_bytes()).into_owned().collect();
                                    assert_eq!(form.get("client_id").unwrap(), "issued-client");
                                    assert_eq!(form.get("resource").unwrap(), RESOURCE);
                                    let refresh = form.get("grant_type").unwrap() == "refresh_token";
                                    if refresh {
                                        state.refreshes += 1;
                                        assert_eq!(form["refresh_token"], "fixture-refresh-one");
                                    } else {
                                        assert_eq!(form["code"], "fixture-code");
                                        assert_eq!(
                                            URL_SAFE_NO_PAD.encode(Sha256::digest(form["code_verifier"].as_bytes())),
                                            state.challenge
                                        );
                                    }
                                    if !refresh && state.reject_code {
                                        status = 400;
                                        json!({"error":"invalid_grant", "error_description":"fixture-private-access-token"}).to_string()
                                    } else if refresh && state.reject_refresh {
                                        status = 401;
                                        json!({"error":"fixture expired"}).to_string()
                                    } else {
                                        let mut header = Header::new(Algorithm::RS256);
                                        header.kid = Some("ephemeral".into());
                                        let jwt=encode(&header,&json!({"sub":if state.wrong_subject {"foreign"} else {"fixture-subject"},"iss":ISSUER,"aud":"issued-client","iat":now(),"exp":now()+3600,"nonce":state.nonce}),&signing).unwrap();
                                        json!({"access_token":if refresh {"fixture-access-two"} else {"fixture-access-one"},"refresh_token":if refresh {"fixture-refresh-two"} else {"fixture-refresh-one"},"id_token":jwt,"token_type":"Bearer","expires_in":3600,"scope":SCOPES}).to_string()
                                    }
                                }
                                "/v1/responses" => {
                                    assert_eq!(auth, "Bearer fixture-access-two");
                                    let body: Value = serde_json::from_str(&body).unwrap();
                                    assert_eq!(body["store"], false);
                                    assert_eq!(body["stream"], true);
                                    if state.inference_status != 0 {
                                        status = state.inference_status;
                                        json!({"error":{"code":"subscription_sharing_unsupported_capability","param":"reasoning.effort","message":"fixture-private-access-token and prompt"}}).to_string()
                                    } else if state.json_response {
                                        json!({"object":"response","id":"fixture-response","status":"completed","output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":"offline reply"}]}]}).to_string()
                                    } else {
                                        content_type = "text/event-stream";
                                        format!(
                                            "data: {}\n\ndata: {}\n\n",
                                            json!({"type":"response.created","response":{"id":"fixture-response"}}),
                                            json!({"type":"response.completed","response":{"id":"fixture-response","status":"completed","output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":"offline reply"}]}]}})
                                        )
                                    }
                                }
                                "/.well-known/openid-configuration" => {
                                    json!({"issuer":ISSUER,"revocation_endpoint":"https://auth.openai.com/revoke"})
                                        .to_string()
                                }
                                "/revoke" => {
                                    let form: HashMap<String, String> =
                                        url::form_urlencoded::parse(body.as_bytes()).into_owned().collect();
                                    assert_eq!(form["token"], "fixture-refresh-two");
                                    "{}".into()
                                }
                                _ => {
                                    status = 404;
                                    "{}".into()
                                }
                            };
                            if path == "/v1/responses" {
                                if let Some(body) = state.inference_body {
                                    output = body.into();
                                }
                                if let Some(mime) = state.inference_mime {
                                    content_type = mime;
                                }
                            }
                            let mut response = Response::builder().status(status).header("x-request-id", "req_fixture");
                            if path != "/v1/responses" || !state.omit_inference_content_type {
                                response = response.header("content-type", content_type);
                            }
                            Ok::<_, Infallible>(response.body(Full::new(Bytes::from(output))).unwrap())
                        }
                    });
                    let _ = http1::Builder::new().serve_connection(TokioIo::new(socket), service).await;
                });
            }
        });
        Self { http, exchange, task, _keys: keys }
    }
    fn auth(&self, root: PathBuf) -> Auth {
        let mut auth = Auth::at(root.canonicalize().unwrap()).unwrap();
        auth.http = self.http.clone();
        auth.inference_http = self.http.clone();
        auth
    }
    async fn login(&self, auth: &Auth) -> Result<AccountSummary> {
        let shared = self.exchange.clone();
        let http = self.http.clone();
        auth.login_with(None, move |url| async move {
            let url = url::Url::parse(&url)?;
            assert_eq!(url.origin().ascii_serialization(), ISSUER);
            let query: HashMap<String, String> = url.query_pairs().into_owned().collect();
            assert_eq!(query["client_id"], "dynamic_agent_client");
            assert_eq!(query["code_challenge_method"], "S256");
            {
                let mut state = shared.lock().unwrap();
                state.nonce = query["nonce"].clone();
                state.challenge = query["code_challenge"].clone();
            }
            let mut callback = url::Url::parse(&query["redirect_uri"])?;
            callback.query_pairs_mut().extend_pairs([
                ("state", query["state"].as_str()),
                ("code", "fixture-code"),
                ("client_id", "issued-client"),
            ]);
            tokio::spawn(async move {
                http.get(callback).send().await.unwrap();
            });
            Ok(())
        })
        .await
    }
}

#[tokio::test]
async fn failed_code_exchange_is_not_reported_as_a_refresh_and_never_leaks_response() {
    let fixture = Fixture::start().await;
    let root = tempfile::tempdir().unwrap();
    let auth = fixture.auth(root.path().to_path_buf());
    fixture.exchange.lock().unwrap().reject_code = true;
    let error = fixture.login(&auth).await.unwrap_err().to_string();
    assert!(error.contains("code exchange") && error.contains("HTTP 400"), "{error}");
    assert!(!error.contains("renewed") && !error.contains("fixture-private"), "{error}");
    assert!(auth.status().await.unwrap().accounts.is_empty());
}

#[tokio::test]
async fn oauth_tls_lifecycle_rotates_once_across_instances_and_revokes_without_leaking() {
    let fixture = Fixture::start().await;
    let root = tempfile::tempdir().unwrap();
    let auth = fixture.auth(root.path().to_path_buf());
    let account = fixture.login(&auth).await.unwrap();
    assert!(account.signed_in);
    assert!(!serde_json::to_string(&auth.status().await.unwrap()).unwrap().contains("fixture-access"));
    {
        let mut store = auth.read().unwrap();
        store.accounts[0].credentials.as_mut().unwrap().expires_at = 0;
        auth.write(&store).unwrap();
    }
    let second = fixture.auth(root.path().to_path_buf());
    let (a, b) = tokio::join!(auth.access_token(), second.access_token());
    assert_eq!(a.unwrap(), "fixture-access-two");
    assert_eq!(b.unwrap(), "fixture-access-two");
    assert_eq!(fixture.exchange.lock().unwrap().refreshes, 1);
    let result =
        inference::complete(&auth, json!({"model":"fixture","input":[{"role":"user","content":"offline test"}]}))
            .await
            .unwrap();
    assert_eq!(result["choices"][0]["message"]["content"], "offline reply");
    assert!(auth.logout(None).await.unwrap());
    assert!(second.access_token().await.is_err());
    let stored = fs::read_to_string(root.path().join("accounts.json")).unwrap();
    assert!(!stored.contains("fixture-access"));
    assert!(!stored.contains("fixture-refresh"));
    let state = fixture.exchange.lock().unwrap();
    assert_eq!(state.requests.iter().filter(|(path, _, _)| path == "/revoke").count(), 1);
    assert!(state.requests.iter().filter(|(path, _, _)| path != "/v1/responses").all(|(_, auth, _)| auth.is_empty()));
}
#[tokio::test]
async fn inference_http_failure_reaches_worker_diagnostics_without_leaks_replay_or_credential_loss() {
    let fixture = Fixture::start().await;
    let root = tempfile::tempdir().unwrap();
    let auth = fixture.auth(root.path().to_path_buf());
    fixture.login(&auth).await.unwrap();
    {
        let mut store = auth.read().unwrap();
        store.accounts[0].credentials.as_mut().unwrap().expires_at = 0;
        auth.write(&store).unwrap();
    }
    auth.access_token().await.unwrap();
    let before = fs::read(root.path().join("accounts.json")).unwrap();
    for status in [400, 403, 429, 503] {
        fixture.exchange.lock().unwrap().inference_status = status;
        let error =
            inference::complete(&auth, json!({"model":"fixture","input":[{"role":"user","content":"offline test"}]}))
                .await
                .unwrap_err();
        let displayed = inference::worker_diagnostic(&error);
        assert!(displayed.contains(&format!("HTTP {status}")) && displayed.contains("req_fixture"), "{displayed}");
        assert!(!displayed.contains("fixture-private") && !displayed.contains("prompt"), "{displayed}");
        assert_eq!(fs::read(root.path().join("accounts.json")).unwrap(), before);
    }
    assert_eq!(
        fixture.exchange.lock().unwrap().requests.iter().filter(|(path, _, _)| path == "/v1/responses").count(),
        4
    );
}
#[tokio::test]
async fn nonstream_completed_json_uses_the_same_request_without_replay_and_html_is_rejected() {
    let fixture = Fixture::start().await;
    let root = tempfile::tempdir().unwrap();
    let auth = fixture.auth(root.path().to_path_buf());
    fixture.login(&auth).await.unwrap();
    {
        let mut store = auth.read().unwrap();
        store.accounts[0].credentials.as_mut().unwrap().expires_at = 0;
        auth.write(&store).unwrap();
    }
    auth.access_token().await.unwrap();
    fixture.exchange.lock().unwrap().json_response = true;
    let request = json!({"model":"fixture","input":[{"role":"user","content":"offline test"}]});
    let output = inference::complete(&auth, request.clone()).await.unwrap();
    assert_eq!(output["choices"][0]["message"]["content"], "offline reply");
    fixture.exchange.lock().unwrap().inference_mime = Some("text/html");
    let error = inference::complete(&auth, request).await.unwrap_err();
    let displayed = inference::worker_diagnostic(&error);
    assert!(displayed.contains("content_type=text/html"), "{displayed}");
    assert!(!displayed.contains("offline reply") && !displayed.contains("fixture-access"));
    assert_eq!(
        fixture.exchange.lock().unwrap().requests.iter().filter(|(path, _, _)| path == "/v1/responses").count(),
        2
    );
}
#[tokio::test]
async fn missing_content_type_accepts_only_complete_responses_and_never_retries_or_leaks_bytes() {
    let fixture = Fixture::start().await;
    let root = tempfile::tempdir().unwrap();
    let auth = fixture.auth(root.path().to_path_buf());
    fixture.login(&auth).await.unwrap();
    {
        let mut store = auth.read().unwrap();
        store.accounts[0].credentials.as_mut().unwrap().expires_at = 0;
        auth.write(&store).unwrap();
    }
    auth.access_token().await.unwrap();
    fixture.exchange.lock().unwrap().omit_inference_content_type = true;
    let request = json!({"model":"fixture","input":[{"role":"user","content":"offline test"}]});
    for json_response in [false, true] {
        fixture.exchange.lock().unwrap().json_response = json_response;
        let result = inference::complete(&auth, request.clone()).await.unwrap();
        assert_eq!(result["choices"][0]["message"]["content"], "offline reply");
    }
    for (body, shape) in [
        ("", "empty"),
        ("<html>PRIVATE_TOKEN_AND_PROMPT</html>", "unrecognized"),
        ("data: {\"type\":\"response.created\",\"response\":{\"id\":\"r1\"}}\n\n", "sse"),
    ] {
        fixture.exchange.lock().unwrap().inference_body = Some(body);
        let error = inference::complete(&auth, request.clone()).await.unwrap_err();
        let text = inference::worker_diagnostic(&error);
        assert!(text.contains("content_type=missing") && text.contains(&format!("format={shape}")), "{text}");
        assert!(!text.contains("PRIVATE_TOKEN_AND_PROMPT"), "{text}");
    }
    assert_eq!(
        fixture.exchange.lock().unwrap().requests.iter().filter(|(path, _, _)| path == "/v1/responses").count(),
        5
    );
}

#[tokio::test]
async fn oauth_refresh_rejection_or_identity_change_never_replaces_saved_credentials() {
    let fixture = Fixture::start().await;
    let root = tempfile::tempdir().unwrap();
    let auth = fixture.auth(root.path().to_path_buf());
    fixture.login(&auth).await.unwrap();
    {
        let mut store = auth.read().unwrap();
        store.accounts[0].credentials.as_mut().unwrap().expires_at = 0;
        auth.write(&store).unwrap();
    }
    let before = fs::read(root.path().join("accounts.json")).unwrap();
    fixture.exchange.lock().unwrap().reject_refresh = true;
    assert!(auth.access_token().await.is_err());
    assert_eq!(fs::read(root.path().join("accounts.json")).unwrap(), before);
    {
        let mut state = fixture.exchange.lock().unwrap();
        state.reject_refresh = false;
        state.wrong_subject = true;
    }
    assert!(auth.access_token().await.is_err());
    assert_eq!(fs::read(root.path().join("accounts.json")).unwrap(), before);
}
