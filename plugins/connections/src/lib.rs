use anyhow::{anyhow, bail, Result};
pub mod inference;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use fs2::FileExt;
use jsonwebtoken::{decode, decode_header, jwk::JwkSet, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};
use uuid::Uuid;

const ISSUER: &str = "https://auth.openai.com";
const TOKEN: &str = "https://auth.openai.com/api/accounts/oauth/token";
const RESOURCE: &str = "https://api.openai.com/v1";
const DIRECT: &str = "chatgpt.tokens.use.direct";
const SCOPES: &str = "openid profile email offline_access resource.invoke chatgpt.tokens.use.direct";
const FAILURE: &str = "ChatGPT authentication could not be completed";

#[cfg(test)]
mod lifecycle_tests;

#[derive(Clone)]
pub struct Auth {
    root: PathBuf,
    http: reqwest::Client,
    inference_http: reqwest::Client,
}

#[derive(Clone, Serialize, Debug)]
pub struct AccountSummary {
    pub id: String,
    pub label: String,
    pub email: Option<String>,
    pub active: bool,
    pub signed_in: bool,
}

#[derive(Serialize, Debug)]
pub struct Status {
    pub active: Option<String>,
    pub accounts: Vec<AccountSummary>,
}

#[derive(Serialize, Deserialize)]
struct Store {
    host: String,
    active: Option<String>,
    accounts: Vec<Account>,
}

#[derive(Clone, Serialize, Deserialize)]
struct Account {
    id: String,
    label: String,
    email: Option<String>,
    issuer: String,
    subject: String,
    client_id: String,
    credentials: Option<Credentials>,
}

#[derive(Clone, Serialize, Deserialize)]
struct Credentials {
    access_token: String,
    refresh_token: Option<String>,
    id_token: String,
    scopes: Vec<String>,
    expires_at: u64,
}

#[derive(Deserialize)]
struct Tokens {
    access_token: String,
    refresh_token: Option<String>,
    id_token: Option<String>,
    token_type: String,
    expires_in: u64,
    scope: Option<String>,
}

#[derive(Clone, Deserialize)]
struct Claims {
    sub: String,
    iss: String,
    exp: u64,
    iat: u64,
    nonce: Option<String>,
    email: Option<String>,
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
}
fn opaque() -> String {
    let mut bytes = Vec::with_capacity(48);
    for _ in 0..3 {
        bytes.extend_from_slice(Uuid::new_v4().as_bytes());
    }
    URL_SAFE_NO_PAD.encode(bytes)
}

fn private_options() -> OpenOptions {
    let mut opts = OpenOptions::new();
    opts.read(true).write(true).create(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
        #[cfg(target_os = "linux")]
        opts.custom_flags(0x20000);
        #[cfg(target_os = "macos")]
        opts.custom_flags(0x100);
    }
    opts
}

fn check_path(path: &std::path::Path) -> Result<()> {
    for ancestor in path.ancestors().filter(|p| !p.as_os_str().is_empty()) {
        match fs::symlink_metadata(ancestor) {
            Ok(meta) if meta.file_type().is_symlink() => bail!("Unsafe ChatGPT credential storage"),
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => bail!("Cannot inspect ChatGPT credential storage"),
        }
    }
    Ok(())
}

impl Auth {
    pub fn new() -> Result<Self> {
        let root = dirs_next::config_dir()
            .ok_or_else(|| anyhow!("No application configuration directory"))?
            .join("i")
            .join("chatgpt");
        Self::at(root)
    }

    pub fn at(root: PathBuf) -> Result<Self> {
        check_path(&root)?;
        fs::create_dir_all(&root).map_err(|_| anyhow!("Cannot create ChatGPT credential storage"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&root, fs::Permissions::from_mode(0o700))
                .map_err(|_| anyhow!("Cannot protect ChatGPT credential storage"))?;
        }
        let http = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|_| anyhow!(FAILURE))?;
        let inference_http = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(15))
            .build()
            .map_err(|_| anyhow!(FAILURE))?;
        let this = Self { root, http, inference_http };
        let lock = this.lock_file("state.lock")?;
        lock.lock_exclusive().map_err(|_| anyhow!("Cannot lock ChatGPT credential storage"))?;
        if !this.root.join("accounts.json").try_exists().map_err(|_| anyhow!(FAILURE))? {
            this.write(&Store { host: format!("urn:uuid:{}", Uuid::new_v4()), active: None, accounts: vec![] })?;
        }
        this.read()?;
        Ok(this)
    }

    fn lock_file(&self, name: &str) -> Result<File> {
        check_path(&self.root)?;
        let path = self.root.join(name);
        check_path(&path)?;
        let file = private_options().open(path).map_err(|_| anyhow!("Cannot lock ChatGPT credential storage"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(fs::Permissions::from_mode(0o600)).map_err(|_| anyhow!(FAILURE))?;
        }
        Ok(file)
    }

    async fn lock(&self, name: &str) -> Result<File> {
        let file = self.lock_file(name)?;
        let started = tokio::time::Instant::now();
        loop {
            match file.try_lock_exclusive() {
                Ok(()) => return Ok(file),
                Err(e)
                    if e.raw_os_error() == fs2::lock_contended_error().raw_os_error()
                        && started.elapsed() < Duration::from_secs(660) =>
                {
                    tokio::time::sleep(Duration::from_millis(50)).await
                }
                Err(_) => bail!("ChatGPT authentication is busy"),
            }
        }
    }

    fn read(&self) -> Result<Store> {
        check_path(&self.root)?;
        let path = self.root.join("accounts.json");
        check_path(&path)?;
        let mut data = String::new();
        let file = private_options().open(path).map_err(|_| anyhow!("Cannot read ChatGPT credential storage"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(fs::Permissions::from_mode(0o600))
                .map_err(|_| anyhow!("Cannot protect ChatGPT credential storage"))?;
        }
        file.take(4 * 1024 * 1024)
            .read_to_string(&mut data)
            .map_err(|_| anyhow!("Cannot read ChatGPT credential storage"))?;
        let store: Store = serde_json::from_str(&data).map_err(|_| anyhow!("Invalid ChatGPT credential storage"))?;
        if !store.host.starts_with("urn:uuid:") || Uuid::parse_str(&store.host[9..]).is_err() {
            bail!("Invalid ChatGPT host identity");
        }
        Ok(store)
    }

    fn write(&self, store: &Store) -> Result<()> {
        check_path(&self.root)?;
        let path = self.root.join("accounts.json");
        check_path(&path)?;
        let mut temp = tempfile::NamedTempFile::new_in(&self.root).map_err(|_| anyhow!(FAILURE))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            temp.as_file().set_permissions(fs::Permissions::from_mode(0o600)).map_err(|_| anyhow!(FAILURE))?;
        }
        serde_json::to_writer(&mut temp, store).map_err(|_| anyhow!(FAILURE))?;
        temp.flush().and_then(|_| temp.as_file().sync_all()).map_err(|_| anyhow!(FAILURE))?;
        temp.persist(path).map_err(|_| anyhow!("Cannot save ChatGPT credentials"))?;
        #[cfg(unix)]
        File::open(&self.root).and_then(|f| f.sync_all()).map_err(|_| anyhow!(FAILURE))?;
        Ok(())
    }

    pub async fn status(&self) -> Result<Status> {
        let _lock = self.lock("state.lock").await?;
        let store = self.read()?;
        Ok(Status {
            accounts: store.accounts.iter().map(|a| summary(a, store.active.as_deref())).collect(),
            active: store.active,
        })
    }

    pub async fn select(&self, id: &str) -> Result<()> {
        let _lock = self.lock("state.lock").await?;
        let mut store = self.read()?;
        if !store.accounts.iter().any(|a| a.id == id && a.credentials.is_some()) {
            bail!("ChatGPT account must be signed in first");
        }
        store.active = Some(id.to_owned());
        self.write(&store)
    }

    pub async fn login(&self, account: Option<&str>) -> Result<AccountSummary> {
        self.login_with(account, |url| async move { open_browser(&url).await }).await
    }

    async fn login_with<F, Fut>(&self, account: Option<&str>, launch: F) -> Result<AccountSummary>
    where
        F: FnOnce(String) -> Fut,
        Fut: std::future::Future<Output = Result<()>>,
    {
        let _oauth_lock = self.lock("oauth.lock").await?;
        let (host, existing) = {
            let _lock = self.lock("state.lock").await?;
            let store = self.read()?;
            let existing = match account {
                Some(id) => Some(
                    store
                        .accounts
                        .iter()
                        .find(|a| a.id == id)
                        .cloned()
                        .ok_or_else(|| anyhow!("Unknown ChatGPT account"))?,
                ),
                None => None,
            };
            (store.host, existing)
        };
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .map_err(|_| anyhow!("Cannot start ChatGPT callback listener"))?;
        let redirect =
            format!("http://127.0.0.1:{}/auth/callback", listener.local_addr().map_err(|_| anyhow!(FAILURE))?.port());
        let state = opaque();
        let nonce = opaque();
        let verifier = opaque();
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        let mut url =
            url::Url::parse("https://auth.openai.com/api/accounts/authorize").map_err(|_| anyhow!(FAILURE))?;
        {
            let mut query = url.query_pairs_mut();
            query.extend_pairs([
                ("client_id", existing.as_ref().map(|a| a.client_id.as_str()).unwrap_or("dynamic_agent_client")),
                ("ext_agent_host_id", &host),
                ("response_type", "code"),
                ("redirect_uri", &redirect),
                ("scope", SCOPES),
                ("resource", RESOURCE),
                ("state", &state),
                ("nonce", &nonce),
                ("code_challenge_method", "S256"),
                ("code_challenge", &challenge),
            ]);
            if let Some(a) = &existing {
                if let Some(c) = &a.credentials {
                    query.append_pair("id_token_hint", &c.id_token);
                }
                if let Some(email) = &a.email {
                    query.append_pair("login_hint", email);
                }
            } else {
                query.append_pair("agent_name_hint", "i / lninl");
            }
        }
        launch(url.to_string()).await?;
        let (code, client_id) = tokio::time::timeout(
            Duration::from_secs(600),
            receive_callback(listener, &state, existing.as_ref().map(|a| a.client_id.as_str())),
        )
        .await
        .map_err(|_| anyhow!("ChatGPT sign-in timed out"))??;
        let tokens = self
            .token(&[
                ("grant_type", "authorization_code"),
                ("client_id", &client_id),
                ("code", &code),
                ("code_verifier", &verifier),
                ("redirect_uri", &redirect),
                ("resource", RESOURCE),
            ])
            .await?;
        let id_token = tokens.id_token.as_deref().ok_or_else(|| anyhow!(FAILURE))?;
        let claims = self.verify(id_token, &client_id, Some(&nonce)).await?;
        if let Some(a) = &existing {
            if a.subject != claims.sub || a.issuer != claims.iss || a.client_id != client_id {
                bail!("ChatGPT account identity did not match");
            }
        }
        let credentials = credentials(tokens, None)?;
        let _lock = self.lock("state.lock").await?;
        let mut store = self.read()?;
        let record = Account {
            id: existing.as_ref().map(|a| a.id.clone()).unwrap_or_else(|| Uuid::new_v4().to_string()),
            label: existing
                .as_ref()
                .map(|a| a.label.clone())
                .unwrap_or_else(|| format!("ChatGPT {}", store.accounts.len() + 1)),
            email: claims.email,
            issuer: claims.iss,
            subject: claims.sub,
            client_id,
            credentials: Some(credentials),
        };
        if let Some(index) = store.accounts.iter().position(|a| a.id == record.id) {
            store.accounts[index] = record.clone();
        } else {
            store.accounts.push(record.clone());
        }
        store.active = Some(record.id.clone());
        self.write(&store)?;
        Ok(summary(&record, store.active.as_deref()))
    }

    async fn token(&self, form: &[(&str, &str)]) -> Result<Tokens> {
        let refresh = form.iter().any(|(key, value)| *key == "grant_type" && *value == "refresh_token");
        let stage = if refresh { "session refresh" } else { "sign-in code exchange" };
        let response = self.http.post(TOKEN).form(form).send().await.map_err(|error| {
            anyhow!(
                "ChatGPT {stage} failed ({}); check network/proxy and try again",
                if error.is_timeout() {
                    "timeout"
                } else if error.is_connect() {
                    "connection failure"
                } else {
                    "transport failure"
                }
            )
        })?;
        if !response.status().is_success() {
            let status = response.status();
            // Never echo the token response or its free-form descriptions: they may contain credentials.
            let hint = match status.as_u16() {
                400 | 401 if refresh => "session renewal was rejected; run /login chatgpt",
                400 | 401 => "authorization code exchange was rejected; start a fresh login; check the helper version and system clock",
                403 => "access was denied; check account/workspace authorization and network/proxy policy",
                429 => "too many requests; wait before trying again",
                500..=599 => "authorization service unavailable; try again later",
                _ => "unexpected authorization response; check network/proxy and helper version",
            };
            return Err(inference::SafeDiagnostic::authentication_http(stage, status.as_u16(), hint).into());
        }
        response.json().await.map_err(|_| anyhow!("ChatGPT {stage} returned an invalid token response"))
    }

    async fn verify(&self, token: &str, client: &str, nonce: Option<&str>) -> Result<Claims> {
        let header = decode_header(token).map_err(|_| anyhow!(FAILURE))?;
        if header.alg != Algorithm::RS256 {
            bail!(FAILURE);
        }
        let kid = header.kid.ok_or_else(|| anyhow!(FAILURE))?;
        let response = self
            .http
            .get("https://auth.openai.com/.well-known/jwks.json")
            .send()
            .await
            .map_err(|_| anyhow!(FAILURE))?;
        if !response.status().is_success() {
            bail!(FAILURE);
        }
        let jwks: JwkSet = response.json().await.map_err(|_| anyhow!(FAILURE))?;
        let key = jwks.find(&kid).ok_or_else(|| anyhow!(FAILURE))?;
        let key = DecodingKey::from_jwk(key).map_err(|_| anyhow!(FAILURE))?;
        verify_signed(token, &key, client, nonce)
    }

    pub async fn access_token(&self) -> Result<String> {
        // This lock spans the exchange and atomic replacement, serializing rotating refreshes across processes.
        let _lock = self.lock("state.lock").await?;
        let mut store = self.read()?;
        let active = store.active.as_deref().ok_or_else(|| anyhow!("Sign in with ChatGPT first"))?;
        let index =
            store.accounts.iter().position(|a| a.id == active).ok_or_else(|| anyhow!("Sign in with ChatGPT first"))?;
        let account = &store.accounts[index];
        let saved = account.credentials.as_ref().ok_or_else(|| anyhow!("Sign in with ChatGPT first"))?;
        require_direct(&saved.scopes)?;
        if saved.expires_at > now().saturating_add(60) {
            return Ok(saved.access_token.clone());
        }
        let refresh =
            saved.refresh_token.as_deref().ok_or_else(|| anyhow!("ChatGPT session expired; sign in again"))?;
        let tokens = self
            .token(&[
                ("grant_type", "refresh_token"),
                ("client_id", &account.client_id),
                ("refresh_token", refresh),
                ("resource", RESOURCE),
            ])
            .await?;
        if let Some(id) = &tokens.id_token {
            let claims = self.verify(id, &account.client_id, None).await?;
            if claims.sub != account.subject || claims.iss != account.issuer {
                bail!("ChatGPT account identity did not match");
            }
        }
        let renewed = credentials(tokens, Some(saved))?;
        let access = renewed.access_token.clone();
        store.accounts[index].credentials = Some(renewed);
        self.write(&store)?;
        require_direct(&store.accounts[index].credentials.as_ref().ok_or_else(|| anyhow!(FAILURE))?.scopes)?;
        Ok(access)
    }

    pub async fn logout(&self, id: Option<&str>) -> Result<bool> {
        self.logout_using(id, Duration::from_secs(20), |refresh, client| async move {
            self.revoke(&refresh, &client).await.unwrap_or(false)
        })
        .await
    }

    async fn logout_using<F, Fut>(&self, id: Option<&str>, remote_timeout: Duration, revoke: F) -> Result<bool>
    where
        F: FnOnce(String, String) -> Fut,
        Fut: std::future::Future<Output = bool>,
    {
        let _oauth = self.lock("oauth.lock").await?;
        let lock = self.lock("state.lock").await?;
        let mut store = self.read()?;
        let id = id.or(store.active.as_deref()).ok_or_else(|| anyhow!("No ChatGPT account selected"))?.to_owned();
        let index = store.accounts.iter().position(|a| a.id == id).ok_or_else(|| anyhow!("Unknown ChatGPT account"))?;
        let a = &store.accounts[index];
        let renewable =
            a.credentials.as_ref().and_then(|c| c.refresh_token.clone()).map(|refresh| (refresh, a.client_id.clone()));
        store.accounts[index].credentials = None;
        if store.active.as_deref() == Some(&id) {
            store.active = None;
        }
        self.write(&store)?;
        drop(lock);
        // Cancellation or a slow remote endpoint must never resurrect local credentials.
        Ok(match renewable {
            Some((refresh, client)) => {
                tokio::time::timeout(remote_timeout, revoke(refresh, client)).await.unwrap_or(false)
            }
            None => false,
        })
    }

    async fn revoke(&self, refresh: &str, client: &str) -> Result<bool> {
        #[derive(Deserialize)]
        struct Discovery {
            issuer: String,
            revocation_endpoint: String,
        }
        let response = self
            .http
            .get("https://auth.openai.com/.well-known/openid-configuration")
            .send()
            .await
            .map_err(|_| anyhow!(FAILURE))?;
        if !response.status().is_success() {
            return Ok(false);
        }
        let discovery: Discovery = response.json().await.map_err(|_| anyhow!(FAILURE))?;
        let endpoint = url::Url::parse(&discovery.revocation_endpoint).map_err(|_| anyhow!(FAILURE))?;
        if discovery.issuer != ISSUER
            || endpoint.scheme() != "https"
            || endpoint.host_str() != Some("auth.openai.com")
            || endpoint.port_or_known_default() != Some(443)
            || !endpoint.username().is_empty()
            || endpoint.password().is_some()
            || endpoint.fragment().is_some()
        {
            return Ok(false);
        }
        for attempt in 0..3 {
            match self
                .http
                .post(endpoint.clone())
                .form(&[("token", refresh), ("token_type_hint", "refresh_token"), ("client_id", client)])
                .send()
                .await
            {
                Ok(r) if r.status() == reqwest::StatusCode::OK => return Ok(true),
                Ok(r) if !r.status().is_server_error() => return Ok(false),
                _ => {}
            }
            if attempt < 2 {
                tokio::time::sleep(Duration::from_millis(200 * (1 << attempt))).await;
            }
        }
        Ok(false)
    }

    pub async fn models(&self) -> Result<serde_json::Value> {
        let token = self.access_token().await?;
        let response = self
            .http
            .get("https://api.openai.com/v1/models")
            .bearer_auth(token)
            .send()
            .await
            .map_err(|_| anyhow!("ChatGPT model catalog unavailable"))?;
        if !response.status().is_success() {
            bail!("ChatGPT model catalog unavailable");
        }
        response.json().await.map_err(|_| anyhow!("ChatGPT model catalog unavailable"))
    }
}

fn summary(a: &Account, active: Option<&str>) -> AccountSummary {
    AccountSummary {
        id: a.id.clone(),
        label: a.label.clone(),
        email: a.email.clone(),
        active: active == Some(a.id.as_str()),
        signed_in: a.credentials.is_some(),
    }
}

fn validate_claims(claims: &Claims, nonce: Option<&str>) -> Result<()> {
    if claims.sub.is_empty()
        || claims.iat > now().saturating_add(5)
        || claims.iat > claims.exp
        || nonce.is_some_and(|expected| claims.nonce.as_deref() != Some(expected))
    {
        bail!(FAILURE);
    }
    Ok(())
}

fn verify_signed(token: &str, key: &DecodingKey, client: &str, nonce: Option<&str>) -> Result<Claims> {
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_issuer(&[ISSUER]);
    validation.set_audience(&[client]);
    validation.set_required_spec_claims(&["sub", "iss", "aud", "exp", "iat"]);
    validation.leeway = 5;
    let claims = decode::<Claims>(token, key, &validation).map_err(|_| anyhow!(FAILURE))?.claims;
    validate_claims(&claims, nonce)?;
    Ok(claims)
}

fn require_direct(scopes: &[String]) -> Result<()> {
    if !scopes.iter().any(|s| s == DIRECT) {
        bail!("ChatGPT plan usage permission was not granted; sign in and authorize plan usage");
    }
    Ok(())
}

fn credentials(tokens: Tokens, saved: Option<&Credentials>) -> Result<Credentials> {
    if !tokens.token_type.eq_ignore_ascii_case("Bearer")
        || tokens.access_token.is_empty()
        || tokens.expires_in == 0
        || tokens.expires_in > 31_536_000
    {
        bail!(FAILURE);
    }
    let scopes = match tokens.scope {
        Some(scope) => scope.split_whitespace().map(str::to_owned).collect(),
        None => saved.map(|c| c.scopes.clone()).unwrap_or_default(),
    };
    Ok(Credentials {
        access_token: tokens.access_token,
        refresh_token: tokens.refresh_token.or_else(|| saved.and_then(|c| c.refresh_token.clone())),
        id_token: tokens.id_token.or_else(|| saved.map(|c| c.id_token.clone())).ok_or_else(|| anyhow!(FAILURE))?,
        scopes,
        expires_at: now().saturating_add(tokens.expires_in),
    })
}

fn callback(target: &str, state: &str, existing: Option<&str>) -> Result<(String, String)> {
    let url = url::Url::parse(&format!("http://127.0.0.1{target}")).map_err(|_| anyhow!(FAILURE))?;
    if url.path() != "/auth/callback" || url.fragment().is_some() {
        bail!(FAILURE);
    }
    let mut params = HashMap::new();
    for (key, value) in url.query_pairs() {
        if params.insert(key.into_owned(), value.into_owned()).is_some() {
            bail!(FAILURE);
        }
    }
    if params.get("state").map(String::as_str) != Some(state) || params.contains_key("error") {
        bail!(FAILURE);
    }
    let code = params.get("code").filter(|s| !s.is_empty()).ok_or_else(|| anyhow!(FAILURE))?.clone();
    let client = match (existing, params.get("client_id")) {
        (Some(expected), Some(received)) if expected != received => bail!(FAILURE),
        (Some(expected), _) => expected.to_owned(),
        (None, Some(received)) if !received.is_empty() && received != "dynamic_agent_client" => received.clone(),
        _ => bail!(FAILURE),
    };
    Ok((code, client))
}

async fn receive_callback(listener: TcpListener, state: &str, existing: Option<&str>) -> Result<(String, String)> {
    loop {
        let (mut stream, peer) = listener.accept().await.map_err(|_| anyhow!(FAILURE))?;
        if !peer.ip().is_loopback() {
            continue;
        }
        let request = tokio::time::timeout(Duration::from_secs(5), async {
            let mut bytes = Vec::new();
            loop {
                let mut chunk = [0u8; 1024];
                let n = stream.read(&mut chunk).await.map_err(|_| anyhow!(FAILURE))?;
                if n == 0 {
                    bail!(FAILURE);
                }
                bytes.extend_from_slice(&chunk[..n]);
                if bytes.len() > 16384 {
                    bail!(FAILURE);
                }
                if bytes.windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
            let text = std::str::from_utf8(&bytes).map_err(|_| anyhow!(FAILURE))?;
            let mut line = text.lines().next().unwrap_or("").split_whitespace();
            if line.next() != Some("GET") {
                bail!(FAILURE);
            }
            Ok(line.next().ok_or_else(|| anyhow!(FAILURE))?.to_owned())
        })
        .await
        .map_err(|_| anyhow!(FAILURE))?;
        let target = request?;
        if !target.starts_with("/auth/callback?") {
            let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await;
            continue;
        }
        // The first callback consumes the listener; success and failure both end this attempt.
        let result = callback(&target, state, existing);
        let body = if result.is_ok() {
            "Authorization callback received. Login is not complete yet: return to i for token exchange and verification. You may close this window."
        } else {
            "Sign-in could not be verified. Return to the application."
        };
        let response = format!("HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\nCache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body);
        let _ = stream.write_all(response.as_bytes()).await;
        return result;
    }
}

async fn open_browser(url: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    let mut command = tokio::process::Command::new("open");
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut c = tokio::process::Command::new("rundll32");
        c.arg("url.dll,FileProtocolHandler");
        c
    };
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let mut command = tokio::process::Command::new("xdg-open");
    let status = tokio::time::timeout(
        Duration::from_secs(30),
        command
            .arg(url)
            .kill_on_drop(true)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status(),
    )
    .await
    .map_err(|_| anyhow!("Cannot open the system browser"))?
    .map_err(|_| anyhow!("Cannot open the system browser"))?;
    if !status.success() {
        bail!("Cannot open the system browser");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tempdir() -> tempfile::TempDir {
        tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap()
    }

    fn account(id: &str, direct: bool) -> Account {
        Account {
            id: id.into(),
            label: format!("ChatGPT {id}"),
            email: Some("same@example.invalid".into()),
            issuer: ISSUER.into(),
            subject: format!("subject-{id}"),
            client_id: format!("client-{id}"),
            credentials: Some(Credentials {
                access_token: format!("secret-access-{id}"),
                refresh_token: None,
                id_token: format!("secret-id-{id}"),
                scopes: if direct { vec![DIRECT.into()] } else { vec!["openid".into(), "email".into()] },
                expires_at: now() + 3600,
            }),
        }
    }

    #[tokio::test]
    async fn logout_is_persisted_before_slow_revocation_and_survives_cancellation() {
        let dir = tempdir();
        let auth = Auth::at(dir.path().join("auth")).unwrap();
        let mut store = auth.read().unwrap();
        let mut saved = account("one", true);
        saved.credentials.as_mut().unwrap().refresh_token = Some("synthetic-refresh".into());
        store.accounts.push(saved);
        store.active = Some("one".into());
        auth.write(&store).unwrap();
        let reached = std::sync::Arc::new(tokio::sync::Notify::new());
        let pending = reached.clone();
        let running = auth.clone();
        let task = tokio::spawn(async move {
            running
                .logout_using(None, Duration::from_secs(20), |_, _| async move {
                    pending.notify_one();
                    std::future::pending::<bool>().await
                })
                .await
        });
        reached.notified().await;
        let cleared = tokio::time::timeout(Duration::from_secs(1), auth.status()).await.unwrap().unwrap();
        assert!(cleared.active.is_none());
        assert!(!cleared.accounts[0].signed_in);
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        let reopened = Auth::at(auth.root.clone()).unwrap();
        assert!(reopened.access_token().await.is_err());
        let persisted = reopened.read().unwrap();
        assert!(persisted.accounts[0].credentials.is_none());
        assert_eq!(persisted.accounts[0].client_id, "client-one");
    }

    #[tokio::test]
    async fn logout_remote_timeout_reports_unconfirmed_without_restoring_credentials() {
        let dir = tempdir();
        let auth = Auth::at(dir.path().join("auth")).unwrap();
        let mut store = auth.read().unwrap();
        let mut saved = account("one", true);
        saved.credentials.as_mut().unwrap().refresh_token = Some("synthetic-refresh".into());
        store.accounts.push(saved);
        store.active = Some("one".into());
        auth.write(&store).unwrap();
        let result = tokio::time::timeout(
            Duration::from_secs(1),
            auth.logout_using(None, Duration::from_millis(5), |_, _| std::future::pending::<bool>()),
        )
        .await
        .unwrap()
        .unwrap();
        assert!(!result);
        assert!(auth.read().unwrap().accounts[0].credentials.is_none());
    }

    #[tokio::test]
    async fn account_selection_logout_and_summaries_are_isolated() {
        let dir = tempdir();
        let auth = Auth::at(dir.path().join("auth")).unwrap();
        let mut store = auth.read().unwrap();
        let a = Uuid::new_v4().to_string();
        let b = Uuid::new_v4().to_string();
        store.accounts = vec![account(&a, true), account(&b, true)];
        auth.write(&store).unwrap();
        auth.select(&a).await.unwrap();
        assert_eq!(auth.access_token().await.unwrap(), format!("secret-access-{a}"));
        let serialized = serde_json::to_string(&auth.status().await.unwrap()).unwrap();
        for forbidden in ["secret-", "client-", "subject-", "refresh_token", "access_token", "id_token"] {
            assert!(!serialized.contains(forbidden));
        }
        assert!(!auth.logout(Some(&a)).await.unwrap());
        let saved = auth.read().unwrap();
        assert!(saved.accounts[0].credentials.is_none());
        assert_eq!(saved.accounts[0].client_id, format!("client-{a}"));
        assert!(saved.accounts[1].credentials.is_some());
        assert!(auth.select(&a).await.is_err());
        auth.select(&b).await.unwrap();
        assert_eq!(auth.access_token().await.unwrap(), format!("secret-access-{b}"));
    }

    #[tokio::test]
    async fn identity_only_grant_cannot_return_access_token() {
        let dir = tempdir();
        let auth = Auth::at(dir.path().join("auth")).unwrap();
        let mut store = auth.read().unwrap();
        store.accounts = vec![account("one", false)];
        store.active = Some("one".into());
        auth.write(&store).unwrap();
        assert!(auth.access_token().await.unwrap_err().to_string().contains("permission"));
    }

    #[tokio::test]
    async fn callback_listener_consumes_exactly_one_validated_callback() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(async move { receive_callback(listener, "state", Some("issued")).await });
        let mut socket = tokio::net::TcpStream::connect(address).await.unwrap();
        socket
            .write_all(b"GET /auth/callback?code=secret-code&state=state HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
            .await
            .unwrap();
        let mut reply = String::new();
        socket.read_to_string(&mut reply).await.unwrap();
        assert!(!reply.contains("secret-code"));
        assert!(reply.contains("no-store"));
        assert!(reply.contains("Login is not complete yet"));
        assert!(reply.contains("token exchange and verification"));
        assert_eq!(task.await.unwrap().unwrap(), ("secret-code".into(), "issued".into()));
        // Concurrent TLS fixtures can reuse the released ephemeral port. A SYN
        // succeeding does not mean this consumed OAuth listener accepted again.
        if let Ok(mut second) = tokio::net::TcpStream::connect(address).await {
            let _=second.write_all(b"GET /auth/callback?code=secret-code&state=state HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n").await;
            let mut bytes = Vec::new();
            let _ = tokio::time::timeout(Duration::from_millis(100), second.take(4096).read_to_end(&mut bytes)).await;
            assert!(!String::from_utf8_lossy(&bytes).contains("Sign-in received"));
        }
    }

    #[tokio::test]
    async fn concurrent_instances_serialize_refresh_lock() {
        let dir = tempdir();
        let auth = Auth::at(dir.path().join("auth")).unwrap();
        let other = Auth::at(auth.root.clone()).unwrap();
        let held = auth.lock("state.lock").await.unwrap();
        assert!(tokio::time::timeout(Duration::from_millis(120), other.lock("state.lock")).await.is_err());
        drop(held);
        assert!(tokio::time::timeout(Duration::from_secs(1), other.lock("state.lock")).await.unwrap().is_ok());
    }

    #[tokio::test]
    async fn jwt_rejects_algorithm_confusion_without_network() {
        let dir = tempdir();
        let auth = Auth::at(dir.path().join("auth")).unwrap();
        let header = jsonwebtoken::Header::new(Algorithm::HS256);
        let token = jsonwebtoken::encode(
            &header,
            &serde_json::json!({"sub":"s","iss":ISSUER,"aud":"issued","iat":now(),"exp":now()+60,"nonce":"n"}),
            &jsonwebtoken::EncodingKey::from_secret(b"not-a-public-key"),
        )
        .unwrap();
        assert!(auth.verify(&token, "issued", Some("n")).await.is_err());
    }

    #[test]
    fn nonce_and_issued_at_are_mandatory_validated_claims() {
        let mut claims = Claims {
            sub: "s".into(),
            iss: ISSUER.into(),
            exp: now() + 600,
            iat: now(),
            nonce: Some("n".into()),
            email: None,
        };
        assert!(validate_claims(&claims, Some("n")).is_ok());
        assert!(validate_claims(&claims, Some("wrong")).is_err());
        claims.nonce = None;
        assert!(validate_claims(&claims, Some("n")).is_err());
        claims.iat = now() + 60;
        assert!(validate_claims(&claims, None).is_err());
        claims.iat = now();
        claims.sub.clear();
        assert!(validate_claims(&claims, None).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn existing_credentials_are_reprotected_before_read() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempdir();
        let auth = Auth::at(dir.path().join("auth")).unwrap();
        let path = auth.root.join("accounts.json");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        auth.read().unwrap();
        assert_eq!(std::fs::metadata(path).unwrap().permissions().mode() & 0o777, 0o600);
    }

    #[test]
    fn signed_jwt_validates_signature_issuer_audience_expiry_and_nonce() {
        // Public test key and precomputed signatures only; no signing key is shipped.
        let fixture: serde_json::Value = serde_json::from_str(include_str!("test-jwt.json")).unwrap();
        let key =
            DecodingKey::from_rsa_components(fixture["n"].as_str().unwrap(), fixture["e"].as_str().unwrap()).unwrap();
        let token = fixture["valid"].as_str().unwrap();
        assert!(verify_signed(token, &key, "issued", Some("nonce")).is_ok());
        for invalid in fixture["invalid"].as_array().unwrap() {
            assert!(verify_signed(invalid.as_str().unwrap(), &key, "issued", Some("nonce")).is_err());
        }
        let mut parts: Vec<_> = token.split('.').map(str::to_owned).collect();
        parts[2] = URL_SAFE_NO_PAD.encode([0; 256]);
        assert!(verify_signed(&parts.join("."), &key, "issued", Some("nonce")).is_err());
    }

    #[tokio::test]
    async fn host_is_stable_and_empty_status_is_safe() {
        let dir = tempdir();
        let auth = Auth::at(dir.path().join("auth")).unwrap();
        let first = auth.read().unwrap();
        assert!(first.host.starts_with("urn:uuid:"));
        assert_eq!(first.host, Auth::at(auth.root.clone()).unwrap().read().unwrap().host);
        let status = auth.status().await.unwrap();
        assert!(status.active.is_none());
        assert!(status.accounts.is_empty());
        assert!(auth.access_token().await.is_err());
    }

    #[test]
    fn callback_rejects_duplicate_state_and_client_replacement() {
        assert!(callback("/auth/callback?state=s&state=s&code=c&client_id=issued", "s", None).is_err());
        assert!(callback("/auth/callback?state=s&code=c&client_id=other", "s", Some("issued")).is_err());
        assert!(callback("/auth/callback?state=s&code=c", "s", None).is_err());
        assert!(callback("/auth/callback?state=x&code=c", "s", Some("issued")).is_err());
        assert!(callback("/auth/callback?state=s&error=access_denied&code=c", "s", Some("issued")).is_err());
        assert_eq!(
            callback("/auth/callback?state=s&code=c", "s", Some("issued")).unwrap(),
            ("c".into(), "issued".into())
        );
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlink_storage_and_writes_private_files() {
        use std::os::unix::{fs::symlink, fs::PermissionsExt};
        let dir = tempdir();
        symlink(dir.path(), dir.path().join("link")).unwrap();
        assert!(Auth::at(dir.path().join("link")).is_err());
        assert!(Auth::at(dir.path().join("link").join("nested-auth")).is_err());
        let auth = Auth::at(dir.path().join("auth")).unwrap();
        assert_eq!(std::fs::metadata(auth.root.join("accounts.json")).unwrap().permissions().mode() & 0o777, 0o600);
        assert_eq!(std::fs::metadata(&auth.root).unwrap().permissions().mode() & 0o777, 0o700);
        std::fs::remove_file(auth.root.join("accounts.json")).unwrap();
        symlink(dir.path().join("unrelated-secret"), auth.root.join("accounts.json")).unwrap();
        assert!(auth.read().is_err());
        assert!(auth
            .write(&Store { host: format!("urn:uuid:{}", Uuid::new_v4()), active: None, accounts: vec![] })
            .is_err());
    }
}
