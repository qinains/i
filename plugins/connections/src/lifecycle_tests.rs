use super::*;
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::{body::Incoming, server::conn::http1, service::service_fn, Request, Response};
use hyper_util::rt::TokioIo;
use jsonwebtoken::{encode, EncodingKey, Header};
use serde_json::{json, Value};
use std::{
    convert::Infallible,
    process::Command,
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
    wrong_subject: bool,
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
fn openssl(args: &[&str]) -> Vec<u8> {
    let mut command = if cfg!(windows) {
        let mut command = Command::new("bash");
        command.args(["-c", "exec openssl \"$@\"", "openssl"]);
        command
    } else {
        Command::new("openssl")
    };
    let output = command.args(args).output().expect("OpenSSL is required for ephemeral OAuth test keys");
    assert!(output.status.success(), "ephemeral test key command failed");
    output.stdout
}
impl Fixture {
    async fn start() -> Self {
        let keys = tempfile::tempdir().unwrap();
        let pem = keys.path().join("ephemeral.pem");
        openssl(&["genrsa", "-out", pem.to_str().unwrap(), "2048"]);
        let signing = EncodingKey::from_rsa_pem(&fs::read(&pem).unwrap()).unwrap();
        let modulus = String::from_utf8(openssl(&["rsa", "-in", pem.to_str().unwrap(), "-noout", "-modulus"])).unwrap();
        let hex = modulus.trim().strip_prefix("Modulus=").unwrap();
        let modulus: Vec<u8> =
            (0..hex.len()).step_by(2).map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap()).collect();
        let jwks = json!({"keys":[{"kty":"RSA","kid":"ephemeral","alg":"RS256","use":"sig","n":URL_SAFE_NO_PAD.encode(modulus),"e":"AQAB"}]});
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
                            let output = match path.as_str() {
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
                                    if refresh && state.reject_refresh {
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
                                    content_type = "text/event-stream";
                                    format!(
                                        "data: {}\n\ndata: {}\n\n",
                                        json!({"type":"response.created","response":{"id":"fixture-response"}}),
                                        json!({"type":"response.completed","response":{"id":"fixture-response","status":"completed","output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":"offline reply"}]}]}})
                                    )
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
                            Ok::<_, Infallible>(
                                Response::builder()
                                    .status(status)
                                    .header("content-type", content_type)
                                    .body(Full::new(Bytes::from(output)))
                                    .unwrap(),
                            )
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
