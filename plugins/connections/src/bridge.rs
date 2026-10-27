use crate::backend::Backend;
use anyhow::{ensure, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use bytes::Bytes;
use http_body_util::{combinators::UnsyncBoxBody, BodyExt, Full, Limited, StreamBody};
use hyper::{
    body::{Frame, Incoming},
    server::conn::http1,
    service::service_fn,
    Method, Request, Response, StatusCode,
};
use hyper_util::rt::TokioIo;
use serde_json::{json, Value};
use std::{convert::Infallible, io, sync::Arc, time::Duration};
use subtle::ConstantTimeEq;
use tokio::{
    net::TcpListener,
    sync::{Mutex, Semaphore},
};
use uuid::Uuid;

type Body = UnsyncBoxBody<Bytes, io::Error>;
struct CancelOnDrop(tokio::task::AbortHandle);
impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}
struct State {
    backend: Backend,
    origin: String,
    host: String,
    token: String,
    login: Mutex<Option<(Uuid, tokio::task::AbortHandle)>>,
    operations: Arc<Semaphore>,
    requests: Arc<Semaphore>,
}
fn origin(value: &str) -> Result<()> {
    let url = url::Url::parse(value)?;
    ensure!(
        url.origin().ascii_serialization() == value
            && (url.scheme() == "https"
                || url.scheme() == "http" && matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]"))),
        "Use an exact HTTPS website origin, or an HTTP loopback origin"
    );
    Ok(())
}
fn allowed(method: &Method, headers: &hyper::HeaderMap, state: &State) -> bool {
    headers.get("host").and_then(|v| v.to_str().ok()) == Some(state.host.as_str())
        && headers.get("origin").and_then(|v| v.to_str().ok()) == Some(state.origin.as_str())
        && (method == Method::OPTIONS
            || headers.get("x-i-local-token").is_some_and(|v| bool::from(v.as_bytes().ct_eq(state.token.as_bytes()))))
}
fn reply(status: StatusCode, value: Value) -> Response<Body> {
    Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .header("cache-control", "no-store")
        .header("x-content-type-options", "nosniff")
        .body(Full::new(Bytes::from(value.to_string())).map_err(|never| match never {}).boxed_unsync())
        .unwrap()
}
fn cors(response: &mut Response<Body>, origin: &str) {
    let headers = response.headers_mut();
    headers.insert("access-control-allow-origin", origin.parse().unwrap());
    headers.insert("vary", "Origin".parse().unwrap());
    headers.insert("access-control-allow-methods", "GET, POST, OPTIONS".parse().unwrap());
    headers.insert("access-control-allow-headers", "content-type, x-i-local-token".parse().unwrap());
    headers.insert("access-control-allow-private-network", "true".parse().unwrap());
}
async fn dispatch(request: Request<Incoming>, state: Arc<State>) -> Result<Response<Body>> {
    if !allowed(request.method(), request.headers(), &state) {
        return Ok(reply(StatusCode::FORBIDDEN, json!({"error":"Local plugin authorization required"})));
    }
    if request.method() == Method::OPTIONS {
        let mut response = reply(StatusCode::NO_CONTENT, Value::Null);
        cors(&mut response, &state.origin);
        return Ok(response);
    }
    let mut response = match process(request, state.clone()).await {
        Ok(value) => value,
        Err(_) => reply(
            StatusCode::BAD_REQUEST,
            json!({"error":"Local connection plugin operation failed. Check the native plugin and sign in again if needed."}),
        ),
    };
    cors(&mut response, &state.origin);
    Ok(response)
}
async fn process(request: Request<Incoming>, state: Arc<State>) -> Result<Response<Body>> {
    let permit = state.requests.clone().try_acquire_owned()?;
    ensure!(request.uri().query().is_none(), "Queries are not supported");
    let path = request.uri().path().to_owned();
    let method = request.method().clone();
    let get = method == Method::GET;
    let post = method == Method::POST;
    if get && path == "/v1/manifest" {
        return Ok(reply(StatusCode::OK, state.backend.manifest()));
    }
    if get && path == "/v1/status" {
        return Ok(reply(StatusCode::OK, state.backend.status().await?));
    }
    if get && path == "/v1/models" {
        return Ok(reply(StatusCode::OK, state.backend.models().await?));
    }
    ensure!(
        post && request.headers().get("content-type").is_some_and(|v| v == "application/json"),
        "Expected JSON POST"
    );
    let bytes =
        tokio::time::timeout(Duration::from_secs(30), Limited::new(request.into_body(), 16 * 1024 * 1024).collect())
            .await?
            .map_err(|_| anyhow::anyhow!("Invalid plugin request body"))?
            .to_bytes();
    let body: Value = serde_json::from_slice(&bytes)?;
    ensure!(body.is_object(), "Expected object");
    if path == "/v1/cancel" {
        ensure!(body.as_object().unwrap().is_empty(), "Unexpected cancel fields");
        if let Some((_, handle)) = state.login.lock().await.take() {
            handle.abort();
        }
        return Ok(reply(StatusCode::OK, json!({})));
    }
    if matches!(path.as_str(), "/v1/login" | "/v1/select" | "/v1/logout") {
        ensure!(body.as_object().unwrap().keys().all(|key| key == "account"), "Unexpected account fields");
        let account =
            body.get("account").map(|v| v.as_str().ok_or_else(|| anyhow::anyhow!("Invalid account"))).transpose()?;
        let operation = state.operations.clone().try_acquire_owned()?;
        if path == "/v1/select" {
            return Ok(reply(
                StatusCode::OK,
                state.backend.select(account.ok_or_else(|| anyhow::anyhow!("Missing account"))?).await?,
            ));
        }
        if path == "/v1/logout" {
            return Ok(reply(StatusCode::OK, state.backend.logout(account).await?));
        }
        let backend = state.backend.clone();
        let account = account.map(str::to_owned);
        let task = tokio::spawn(async move {
            let _operation = operation;
            backend.login(account.as_deref()).await
        });
        let _cancel_on_disconnect = CancelOnDrop(task.abort_handle());
        let login_id = Uuid::new_v4();
        *state.login.lock().await = Some((login_id, task.abort_handle()));
        let result = tokio::time::timeout(Duration::from_secs(600), task).await;
        let mut pending = state.login.lock().await;
        if pending.as_ref().is_some_and(|(id, _)| *id == login_id) {
            if let Some((_, handle)) = pending.take() {
                handle.abort();
            }
        }
        drop(pending);
        return Ok(reply(StatusCode::OK, result???));
    }
    let protocol = state.backend.manifest()["protocol"].as_str().unwrap().to_owned();
    ensure!(path == if protocol == "anthropic" { "/v1/messages" } else { "/v1/responses" }, "Unknown plugin route");
    let response = state.backend.response(body).await?;
    ensure!(
        response
            .headers()
            .get("content-type")
            .is_some_and(|value| value.to_str().unwrap_or("").starts_with("text/event-stream")),
        "Expected provider stream"
    );
    let stream = futures_util::stream::unfold(Some((response, 0usize, permit)), |state| async move {
        let (mut response, size, permit) = state?;
        match tokio::time::timeout(Duration::from_secs(1800), response.chunk()).await {
            Ok(Ok(Some(chunk))) if size + chunk.len() <= 48 * 1024 * 1024 => {
                Some((Ok(Frame::data(chunk.clone())), Some((response, size + chunk.len(), permit))))
            }
            Ok(Ok(None)) => None,
            _ => Some((Err(io::Error::other("Provider stream interrupted")), None)),
        }
    });
    Ok(Response::builder()
        .header("content-type", "text/event-stream")
        .header("cache-control", "no-store")
        .header("x-content-type-options", "nosniff")
        .body(StreamBody::new(stream).boxed_unsync())?)
}
pub async fn serve(backend: Backend, website: String, port: u16) -> Result<()> {
    origin(&website)?;
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)).await?;
    let host = listener.local_addr()?.to_string();
    let token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let pair = URL_SAFE_NO_PAD.encode(json!({"url":format!("http://{host}"),"token":token}).to_string());
    println!("Open this private pairing link on this computer (do not share):\n{website}/app/#connection-plugin={pair}\nKeep this plugin running. Press Ctrl+C to stop.");
    let state = Arc::new(State {
        backend,
        origin: website,
        host,
        token,
        login: Mutex::new(None),
        operations: Arc::new(Semaphore::new(1)),
        requests: Arc::new(Semaphore::new(4)),
    });
    loop {
        tokio::select! {
            result=listener.accept()=>{let (socket,_)=result?;let state=state.clone();tokio::spawn(async move {
                let service=service_fn(move|request|{let state=state.clone();async move {
                    let response=dispatch(request,state).await.unwrap_or_else(|_|reply(StatusCode::BAD_REQUEST,json!({"error":"Invalid plugin request"})));
                    Ok::<_,Infallible>(response)
                }});
                let _=http1::Builder::new().max_headers(32).max_buf_size(8192).serve_connection(TokioIo::new(socket),service).await;
            });},
            _=tokio::signal::ctrl_c()=>{if let Some((_,task))=state.login.lock().await.take(){task.abort();}return Ok(());}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn disconnect_cancels_detached_login_task() {
        let task = tokio::spawn(std::future::pending::<()>());
        let guard = CancelOnDrop(task.abort_handle());
        drop(guard);
        assert!(task.await.unwrap_err().is_cancelled());
    }
    #[test]
    fn origins_are_exact_and_loopback_is_not_an_arbitrary_proxy() {
        for value in ["https://i.lninl.com", "http://127.0.0.1:4173", "http://localhost:8080"] {
            assert!(origin(value).is_ok());
        }
        for value in [
            "https://i.lninl.com/",
            "https://i.lninl.com@app.invalid/",
            "null",
            "http://example.com",
            "https://i.lninl.com?x=1",
        ] {
            assert!(origin(value).is_err());
        }
    }
    #[test]
    fn host_origin_and_capability_are_all_required() {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().canonicalize().unwrap();
        let state = State {
            backend: Backend::Chatgpt(i_chatgpt::Auth::at(path).unwrap()),
            origin: "https://i.lninl.com".into(),
            host: "127.0.0.1:1456".into(),
            token: "synthetic-capability-test".into(),
            login: Mutex::new(None),
            operations: Arc::new(Semaphore::new(1)),
            requests: Arc::new(Semaphore::new(4)),
        };
        let mut headers = hyper::HeaderMap::new();
        headers.insert("host", state.host.parse().unwrap());
        headers.insert("origin", state.origin.parse().unwrap());
        assert!(!allowed(&Method::GET, &headers, &state));
        assert!(allowed(&Method::OPTIONS, &headers, &state));
        headers.insert("x-i-local-token", state.token.parse().unwrap());
        assert!(allowed(&Method::GET, &headers, &state));
        headers.insert("origin", "https://untrusted.invalid".parse().unwrap());
        assert!(!allowed(&Method::GET, &headers, &state));
        headers.insert("origin", state.origin.parse().unwrap());
        headers.insert("host", "attacker.invalid:1456".parse().unwrap());
        assert!(!allowed(&Method::GET, &headers, &state));
    }
}
