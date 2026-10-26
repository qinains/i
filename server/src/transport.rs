use bytes::Bytes;
use futures_util::{Stream, future::BoxFuture, stream};
use hyper::{HeaderMap, StatusCode, header::HeaderValue};
use std::{
    net::{IpAddr, SocketAddr},
    pin::Pin,
    sync::Arc,
    time::Duration,
};
use url::Url;

pub(crate) type Resolver = Arc<dyn Fn(String) -> BoxFuture<'static, Result<Vec<IpAddr>, ()>> + Send + Sync>;
pub(crate) type Transport = Arc<dyn Fn(UpstreamRequest) -> BoxFuture<'static, Result<Upstream, ()>> + Send + Sync>;

pub(crate) struct UpstreamRequest {
    pub url: Url,
    pub pinned: SocketAddr,
    pub headers: HeaderMap,
    pub body: Bytes,
    pub inactivity: Duration,
}

pub(crate) struct Upstream {
    pub status: StatusCode,
    pub content_type: HeaderValue,
    pub length: Option<u64>,
    pub chunks: Pin<Box<dyn Stream<Item = Result<Bytes, ()>> + Send>>,
}

pub(crate) fn resolver() -> Resolver {
    Arc::new(|host| {
        Box::pin(async move {
            let host = host.trim_start_matches('[').trim_end_matches(']');
            if let Ok(address) = host.parse() {
                return Ok(vec![address]);
            }
            let addresses = tokio::net::lookup_host((host, 443)).await.map_err(|_| ())?;
            Ok(addresses.map(|address| address.ip()).collect())
        })
    })
}

pub(crate) fn transport() -> Transport {
    Arc::new(|request| {
        Box::pin(async move {
            let host = request.url.host_str().ok_or(())?;
            let client = reqwest::Client::builder()
                .use_rustls_tls()
                .https_only(true)
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .referer(false)
                .retry(reqwest::retry::never())
                .no_gzip()
                .no_brotli()
                .no_deflate()
                .no_zstd()
                .connect_timeout(request.inactivity)
                .read_timeout(request.inactivity)
                .pool_max_idle_per_host(0)
                .resolve(host, request.pinned)
                .build()
                .map_err(|_| ())?;
            let response =
                client.post(request.url).headers(request.headers).body(request.body).send().await.map_err(|_| ())?;
            if response.status().is_redirection()
                || response.headers().get("content-encoding").is_some_and(|value| value != "identity")
            {
                return Err(());
            }
            let status = response.status();
            let content_type = response
                .headers()
                .get("content-type")
                .cloned()
                .unwrap_or(HeaderValue::from_static("application/octet-stream"));
            let length = response.content_length();
            let chunks = stream::try_unfold(response, |mut response| async move {
                match response.chunk().await.map_err(|_| ())? {
                    Some(chunk) => Ok(Some((chunk, response))),
                    None => Ok(None),
                }
            });
            Ok(Upstream { status, content_type, length, chunks: Box::pin(chunks) })
        })
    })
}
