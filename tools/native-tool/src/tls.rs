use anyhow::{Result, ensure};
use serde_json::{Value, json};
use std::{
    fs,
    io::Read,
    net::{TcpListener, TcpStream},
    path::Path,
    process::Command,
    sync::Arc,
    thread,
    time::{Duration, Instant},
};
fn client_hello(socket: &TcpStream) -> Result<bool> {
    socket.set_nonblocking(false)?;
    socket.set_read_timeout(Some(Duration::from_secs(5)))?;
    let mut first = [0];
    Ok(socket.peek(&mut first).is_ok_and(|n| n == 1 && first[0] == 0x16))
}
pub fn check(binary: &Path, web: bool) -> Result<()> {
    for worker in [false, true].into_iter().filter(|worker| !*worker || web) {
        let temp = tempfile::tempdir()?;
        let root = temp.path().canonicalize()?;
        let cert = rcgen::generate_simple_self_signed(vec!["localhost".into(), "127.0.0.1".into()])?;
        let tls = rustls::ServerConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .with_safe_default_protocol_versions()?
            .with_no_client_auth()
            .with_single_cert(
                vec![cert.cert.der().clone()],
                rustls::pki_types::PrivatePkcs8KeyDer::from(cert.signing_key.serialize_der()).into(),
            )?;
        let server = TcpListener::bind("127.0.0.1:0")?;
        let port = server.local_addr()?.port();
        server.set_nonblocking(true)?;
        let task = thread::spawn(move || -> Result<(bool, bool)> {
            let start = Instant::now();
            let mut socket = loop {
                match server.accept() {
                    Ok((socket, _)) => break socket,
                    Err(_) => {
                        ensure!(start.elapsed() < Duration::from_secs(12), "TLS fixture received no connection");
                        thread::sleep(Duration::from_millis(10));
                    }
                }
            };
            let hello = client_hello(&socket)?;
            let mut connection = rustls::ServerConnection::new(Arc::new(tls))?;
            let mut tls = rustls::Stream::new(&mut connection, &mut socket);
            let mut data = [0];
            let http = tls.read(&mut data).is_ok_and(|n| n > 0);
            Ok((hello, http))
        });
        let mut command = Command::new(binary);
        crate::isolated(&mut command, &root);
        let input = if worker {
            let config = root.join("web.json");
            fs::write(&config, json!({"allow_private_network":true}).to_string())?;
            command.args(["--runtime", "web", "--config", config.to_str().unwrap()]);
            format!(
                "{}\n",
                json!({"id":1,"event":"tool","workspace":root,"data":{"name":"web_fetch","arguments":{"url":format!("https://127.0.0.1:{port}")}}})
            )
        } else {
            let config = root.join("api.json");
            fs::write(&config,json!({"base_url":format!("https://127.0.0.1:{port}"),"model":"offline-fixture","api_key_env":"I_TLS_NO_KEY","proxy":"","network_retries":5}).to_string())?;
            command.args(["--config", config.to_str().unwrap(), "-C", root.to_str().unwrap(), "offline fixture"]);
            String::new()
        };
        let output = crate::run(&mut command, input.as_bytes())?;
        let (hello, http) = task.join().unwrap()?;
        ensure!(hello && !http, "TLS fixture requires ClientHello and no HTTP over untrusted TLS");
        let stderr = String::from_utf8(output.stderr)?;
        ensure!(!stderr.contains("panicked") && !stderr.contains("network retry"), "Invalid TLS failure path");
        let diagnostic = if worker {
            ensure!(output.status.success(), "Web worker exited abnormally");
            let reply: Value = serde_json::from_slice(&output.stdout)?;
            ensure!(reply["id"] == 1, "Invalid worker reply");
            reply["error"].as_str().unwrap_or("").to_owned()
        } else {
            ensure!(output.status.code() == Some(1), "Expected certificate rejection");
            stderr
        };
        ensure!(
            ["certificate", "cert_", "trust", "issuer"]
                .iter()
                .any(|term| diagnostic.to_ascii_lowercase().contains(term)),
            "Failure was not certificate rejection: {diagnostic}"
        );
    }
    println!("PASS: TLS ClientHello, certificate rejection, no HTTP, panic or retry");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    #[test]
    fn delayed_client_hello_is_not_mistaken_for_plain_http() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let client = thread::spawn(move || {
            let mut socket = TcpStream::connect(address).unwrap();
            thread::sleep(Duration::from_millis(100));
            socket.write_all(&[0x16]).unwrap();
        });
        let (socket, _) = listener.accept().unwrap();
        socket.set_nonblocking(true).unwrap();
        let hello = client_hello(&socket).unwrap();
        client.join().unwrap();
        assert!(hello);
    }
}
