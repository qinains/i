use super::*;
use std::time::SystemTime;

pub struct ApiResponse {
    pub status: u16,
    pub headers: http::HeaderMap,
    pub body: Vec<u8>,
}

impl ApiResponse {
    pub fn status_obj(&self) -> http::StatusCode {
        http::StatusCode::from_u16(self.status).unwrap_or(http::StatusCode::INTERNAL_SERVER_ERROR)
    }
}

type Reply = Result<ApiResponse>;

pub fn client(proxy: Option<&str>) -> Result<reqwest::Client> {
    let mut builder = reqwest::Client::builder()
        .timeout(Duration::from_secs(180))
        .connect_timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none());

    if let Some(proxy_url) = proxy {
        builder = builder.no_proxy();
        if !proxy_url.is_empty() {
            let parsed = reqwest::Url::parse(proxy_url).context("invalid proxy URL")?;
            ensure!(
                matches!(parsed.scheme(), "http" | "https" | "socks5" | "socks5h") && parsed.host_str().is_some(),
                "unsupported proxy scheme or missing host"
            );
            builder = builder.proxy(reqwest::Proxy::all(proxy_url)?);
        }
    }

    Ok(builder.build()?)
}

fn transient(error: &anyhow::Error) -> bool {
    if error.chain().any(tls_failure) {
        return false;
    }
    error
        .downcast_ref::<reqwest::Error>()
        .map(|e| e.is_timeout() || e.is_connect() || e.is_request() || e.is_body() || e.is_decode())
        .unwrap_or(false)
}

fn tls_failure(mut error: &(dyn std::error::Error + 'static)) -> bool {
    // io::Error::source skips nested wrappers; inspect their payload explicitly.
    for _ in 0..32 {
        if error.is::<rustls::Error>() {
            return true;
        }
        let next = error
            .downcast_ref::<std::io::Error>()
            .and_then(|inner| inner.get_ref().map(|value| value as &(dyn std::error::Error + 'static)))
            .or_else(|| error.source());
        let Some(next) = next else { return false };
        error = next;
    }
    false
}

async fn read_response(mut response: reqwest::Response) -> Result<ApiResponse> {
    let status = response.status().as_u16();
    let headers = response.headers().clone();
    const LIMIT: usize = 48 * 1024 * 1024;
    ensure!(response.content_length().is_none_or(|length| length <= LIMIT as u64), "API response exceeds 48 MiB");
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        ensure!(body.len().saturating_add(chunk.len()) <= LIMIT, "API response exceeds 48 MiB");
        body.extend_from_slice(&chunk);
    }
    Ok(ApiResponse { status, headers, body })
}

pub async fn exchange(client: &reqwest::Client, request: http::Request<Vec<u8>>, remaining: Duration) -> Reply {
    ensure!(!remaining.is_zero(), "model request deadline exceeded");
    let mut reqwest_request = reqwest::Request::try_from(request)?;
    *reqwest_request.timeout_mut() = Some(remaining);
    let response = client.execute(reqwest_request).await?;
    read_response(response).await
}

pub async fn send(
    client: &reqwest::Client,
    request: http::Request<Vec<u8>>,
    retries: u8,
    mut on_retry: impl FnMut() -> Result<()>,
) -> Reply {
    let deadline = Instant::now() + Duration::from_secs(180);

    for attempt in 0..=retries {
        let remaining = deadline.checked_duration_since(Instant::now()).context("model reconnect deadline exceeded")?;

        let result = exchange(client, request.clone(), remaining).await;
        let mut delay = Duration::from_secs(1 << attempt);

        match &result {
            Ok(response) => {
                if !matches!(response.status, 408 | 429 | 500 | 502 | 503 | 504 | 529) {
                    return result;
                }
                if let Some(value) = response.headers.get("retry-after").and_then(|v| v.to_str().ok()) {
                    let wait = value.parse().ok().map(Duration::from_secs).or_else(|| {
                        httpdate::parse_http_date(value)
                            .ok()
                            .map(|date| date.duration_since(SystemTime::now()).unwrap_or_default())
                    });
                    delay = delay.max(wait.unwrap_or_default());
                }
            }
            Err(error) if !transient(error) => return result,
            Err(_) => {}
        }

        if attempt == retries || deadline.saturating_duration_since(Instant::now()) <= delay {
            return result.context("model request failed; reconnect exhausted");
        }

        let (retry, seconds) = (attempt + 1, delay.as_secs());
        eprintln!("[network retry {retry}/{retries} in {seconds}s; possible repeat billing; tools NOT replayed]");
        tokio::time::sleep(delay).await;
        on_retry()?;
    }

    unreachable!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_io_certificate_errors_are_permanent() {
        let certificate = rustls::Error::InvalidCertificate(rustls::CertificateError::UnknownIssuer);
        let inner = std::io::Error::new(std::io::ErrorKind::InvalidData, certificate);
        let outer = std::io::Error::other(inner);
        assert!(tls_failure(&outer));
        assert!(!tls_failure(&std::io::Error::from(std::io::ErrorKind::ConnectionRefused)));
    }
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    async fn endpoint(header: &'static str, delay: Duration) -> (String, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut bytes = [0; 4096];
            assert!(socket.read(&mut bytes).await.unwrap() > 0);
            tokio::time::sleep(delay).await;
            socket.write_all(header.as_bytes()).await.unwrap();
        });
        (url, task)
    }
    #[tokio::test]
    async fn exchange_respects_the_remaining_logical_deadline() {
        let (url, task) = endpoint("HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}", Duration::from_millis(300)).await;
        let request = http::Request::builder().uri(url).body(Vec::new()).unwrap();
        let result = exchange(&client(Some("")).unwrap(), request, Duration::from_millis(40)).await;
        task.abort();
        assert!(result.is_err(), "remaining request budget must not be ignored");
    }
    #[tokio::test]
    async fn declared_oversized_body_is_rejected_before_collecting_it() {
        let (url, task) = endpoint("HTTP/1.1 200 OK\r\nContent-Length: 50331649\r\n\r\n", Duration::ZERO).await;
        let request = http::Request::builder().uri(url).body(Vec::new()).unwrap();
        let error = exchange(&client(Some("")).unwrap(), request, Duration::from_secs(1)).await.err().unwrap();
        task.abort();
        assert!(error.to_string().contains("48 MiB"), "{error:#}");
    }
}
