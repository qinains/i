mod activity;
mod config;
mod transport;
pub use config::Config;

use activity::Activity;
use bytes::{Bytes, BytesMut};
use futures_util::{StreamExt, stream};
use http_body_util::{BodyExt, Full, StreamBody, combinators::UnsyncBoxBody};
use hyper::{
    Request, Response, StatusCode,
    body::{Frame, Incoming},
    header::{HeaderMap, HeaderValue},
    service::service_fn,
};
use hyper_util::rt::{TokioIo, TokioTimer};
use std::{
    convert::Infallible,
    future::Future,
    io,
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::{
    net::TcpListener,
    sync::{Semaphore, watch},
    task::JoinSet,
    time::timeout,
};
use transport::{Resolver, Transport, UpstreamRequest};

type Body = UnsyncBoxBody<Bytes, io::Error>;
type Reply = Response<Body>;

pub struct Relay {
    config: Config,
    slots: Arc<Semaphore>,
    rate: Mutex<(Instant, usize)>,
    resolve: Resolver,
    connect: Transport,
}

impl Relay {
    pub fn new(config: Config) -> Self {
        let slots = Arc::new(Semaphore::new(config.limits.concurrency));
        Self {
            config,
            slots,
            rate: Mutex::new((Instant::now(), 0)),
            resolve: transport::resolver(),
            connect: transport::transport(),
        }
    }

    async fn request(&self, mut request: Request<Incoming>, connection: watch::Sender<bool>) -> Reply {
        if request.uri().path_and_query().map(|path| path.as_str()) != Some(&self.config.route) {
            return reply(404, "Not found");
        }
        if request.method() != "POST" {
            return reply(405, "POST required");
        }
        let headers = request.headers();
        for key in
            ["origin", "x-harness-relay-token", "x-harness-upstream", "content-type", "authorization", "x-api-key"]
        {
            if headers.get_all(key).iter().count() > 1 {
                return reply(400, "Duplicate request header");
            }
        }
        if headers.get("origin").and_then(|value| value.to_str().ok()) != Some(&self.config.origin) {
            return reply(403, "Origin denied");
        }
        if !self.config.authorized(headers.get("x-harness-relay-token").map_or(&[], HeaderValue::as_bytes)) {
            return reply(401, "Relay authentication failed");
        }
        if !headers.get("content-type").and_then(|value| value.to_str().ok()).is_some_and(|value| {
            value.split(';').next().is_some_and(|mime| mime.trim().eq_ignore_ascii_case("application/json"))
        }) {
            return reply(415, "JSON required");
        }
        let target = headers
            .get("x-harness-upstream")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| config::endpoint(value).ok())
            .and_then(|url| self.config.targets.get(url.as_str()));
        let Some(target) = target else {
            return reply(403, "Upstream denied");
        };
        let Ok(permit) = self.slots.clone().try_acquire_owned() else {
            return reply(429, "Relay limit reached");
        };
        {
            let Ok(mut rate) = self.rate.lock() else {
                return reply(503, "Relay unavailable");
            };
            if rate.0.elapsed() >= Duration::from_secs(60) {
                *rate = (Instant::now(), 0);
            }
            if rate.1 >= self.config.limits.rate {
                return reply(429, "Relay limit reached");
            }
            rate.1 += 1;
        }
        let activity = Activity::new(self.config.limits.inactivity, connection);
        let mut forward = HeaderMap::new();
        forward.insert("content-type", HeaderValue::from_static("application/json"));
        forward.insert("accept", HeaderValue::from_static("text/event-stream, application/json"));
        forward.insert("accept-encoding", HeaderValue::from_static("identity"));
        let credential = if target.path().ends_with("/v1/messages") {
            forward.insert("anthropic-version", HeaderValue::from_static("2023-06-01"));
            "x-api-key"
        } else {
            "authorization"
        };
        if let Some(value) = headers.get(credential) {
            let mut value = value.clone();
            value.set_sensitive(true);
            forward.insert(credential, value);
        }
        let bytes = match timeout(
            self.config.limits.body_timeout,
            read_body(request.body_mut(), &activity, self.config.limits.bytes),
        )
        .await
        {
            Ok(Ok(body)) => body,
            Ok(Err(status)) => return reply(status, "Invalid or oversized request body"),
            Err(_) => return reply(408, "Request body timed out"),
        };
        if serde_json::from_slice::<serde_json::Value>(&bytes).is_err() {
            return reply(400, "Invalid JSON");
        }
        let host = target.host_str().expect("validated endpoint has a host").to_owned();
        let addresses = match activity.run((self.resolve)(host)).await {
            Ok(Ok(addresses)) if !addresses.is_empty() && addresses.iter().all(|ip| config::is_public(*ip)) => {
                addresses
            }
            Err(status) => return reply(status, "Relay request failed"),
            _ => return reply(502, "Relay request failed"),
        };
        let send = UpstreamRequest {
            url: target.clone(),
            pinned: SocketAddr::new(addresses[0], 443),
            headers: forward,
            body: bytes,
            inactivity: self.config.limits.inactivity,
        };
        let upstream = match activity.run((self.connect)(send)).await {
            Ok(Ok(upstream))
                if !upstream.status.is_redirection()
                    && upstream.length.is_none_or(|length| length <= self.config.limits.bytes as u64) =>
            {
                upstream
            }
            Err(status) => return reply(status, "Relay request failed"),
            _ => return reply(502, "Relay request failed"),
        };
        activity.touch();
        activity.begin_stream();
        let status = upstream.status;
        let content_type = upstream.content_type;
        let limit = self.config.limits.bytes;
        let stream = stream::try_unfold(
            (upstream.chunks, activity, permit, 0usize),
            move |(mut source, activity, permit, received)| async move {
                match activity.run(source.next()).await {
                    Ok(Some(Ok(chunk))) if chunk.len() <= limit.saturating_sub(received) => {
                        if !chunk.is_empty() {
                            activity.touch();
                        }
                        let received = received + chunk.len();
                        Ok(Some((Frame::data(chunk), (source, activity, permit, received))))
                    }
                    Ok(None) => Ok(None),
                    _ => Err(io::Error::other("Relay stream interrupted")),
                }
            },
        );
        let mut response = Response::new(StreamBody::new(stream).boxed_unsync());
        *response.status_mut() = status;
        response.headers_mut().insert("content-type", content_type);
        response.headers_mut().insert("cache-control", HeaderValue::from_static("no-store"));
        response.headers_mut().insert("x-accel-buffering", HeaderValue::from_static("no"));
        response
    }
}

async fn read_body(body: &mut Incoming, activity: &Activity, limit: usize) -> Result<Bytes, u16> {
    let mut bytes = BytesMut::new();
    while let Some(frame) = activity.run(body.frame()).await? {
        let frame = frame.map_err(|_| 400u16)?;
        if let Ok(chunk) = frame.into_data() {
            if chunk.len() > limit.saturating_sub(bytes.len()) {
                return Err(413);
            }
            if !chunk.is_empty() {
                activity.touch();
            }
            bytes.extend_from_slice(&chunk);
        }
    }
    Ok(bytes.freeze())
}

fn reply(status: u16, text: &'static str) -> Reply {
    let mut response = Response::new(
        Full::new(Bytes::from_static(text.as_bytes())).map_err(|never: Infallible| match never {}).boxed_unsync(),
    );
    *response.status_mut() = StatusCode::from_u16(status).expect("fixed HTTP status");
    response.headers_mut().insert("content-type", HeaderValue::from_static("text/plain; charset=utf-8"));
    response.headers_mut().insert("cache-control", HeaderValue::from_static("no-store"));
    response
}

struct Disconnect(watch::Sender<bool>);
impl Drop for Disconnect {
    fn drop(&mut self) {
        let _ = self.0.send(true);
    }
}

pub async fn serve(listener: TcpListener, relay: Arc<Relay>, shutdown: impl Future<Output = ()>) -> io::Result<()> {
    let mut tasks = JoinSet::new();
    let connections = Arc::new(Semaphore::new(128));
    tokio::pin!(shutdown);
    loop {
        tokio::select! {
            _ = &mut shutdown => break,
            Some(_) = tasks.join_next(), if !tasks.is_empty() => {},
            incoming = listener.accept() => {
                let (socket, _) = incoming?;
                let Ok(slot) = connections.clone().try_acquire_owned() else { continue; };
                let relay = relay.clone();
                tasks.spawn(async move {
                    let _slot = slot;
                    let (cancel, mut cancelled) = watch::channel(false);
                    let _disconnect = Disconnect(cancel.clone());
                    let service = service_fn(move |request| {
                        let relay = relay.clone(); let cancel = cancel.clone();
                        async move { Ok::<_, Infallible>(relay.request(request, cancel).await) }
                    });
                    let mut builder = hyper::server::conn::http1::Builder::new();
                    builder.timer(TokioTimer::new()).header_read_timeout(Duration::from_secs(180)).max_buf_size(16384).max_headers(64);
                    tokio::select! {
                        _ = builder.serve_connection(TokioIo::new(socket), service) => {},
                        _ = cancelled.changed() => {},
                    }
                });
            }
        }
    }
    tasks.abort_all();
    while tasks.join_next().await.is_some() {}
    Ok(())
}

#[cfg(test)]
mod tests;
