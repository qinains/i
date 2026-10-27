use anyhow::{Context, Result, ensure};
use std::{
    fs,
    io::{Read, Write},
    net::{Ipv4Addr, TcpListener, TcpStream},
    path::{Component, Path},
    time::Duration,
};
pub struct Asset {
    pub bytes: Vec<u8>,
    pub mime: &'static str,
}
pub fn asset(directory: &Path, target: &str) -> Result<Asset> {
    ensure!(target.starts_with('/') && !target.starts_with("//") && !target.contains('\\'), "invalid preview target");
    let path = target.split('?').next().unwrap();
    let mut decoded = vec![];
    let bytes = path.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            ensure!(index + 2 < bytes.len(), "invalid percent encoding");
            let high = (bytes[index + 1] as char).to_digit(16).context("invalid percent encoding")?;
            let low = (bytes[index + 2] as char).to_digit(16).context("invalid percent encoding")?;
            decoded.push((high * 16 + low) as u8);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    let decoded = std::str::from_utf8(&decoded)?;
    ensure!(!decoded.contains(['\0', '\\']) && !decoded.starts_with("//"), "invalid preview path");
    let relative = Path::new(decoded.trim_start_matches('/'));
    ensure!(relative.components().all(|part| matches!(part, Component::Normal(_))), "invalid preview path");
    let root = fs::canonicalize(directory)?;
    let mut path = root.join(relative);
    if path.is_dir() {
        path.push("index.html");
    }
    let path = fs::canonicalize(path)?;
    ensure!(path.starts_with(&root) && path.is_file(), "preview path escapes directory");
    let mime = match path.extension().and_then(|ext| ext.to_str()).unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json",
        "wasm" => "application/wasm",
        "webmanifest" => "application/manifest+json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "ico" => "image/x-icon",
        "zip" => "application/zip",
        "gz" => "application/gzip",
        "xml" => "application/xml",
        "txt" | "md" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    };
    Ok(Asset { bytes: fs::read(path)?, mime })
}
fn response(stream: &mut TcpStream, status: &str, mime: &str, bytes: &[u8], head: bool) -> Result<()> {
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nX-Content-Type-Options: nosniff\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        bytes.len()
    )?;
    if !head {
        stream.write_all(bytes)?;
    }
    Ok(())
}
fn connection(mut stream: TcpStream, directory: &Path, port: u16) -> Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    let mut bytes = vec![];
    let mut chunk = [0; 1024];
    while !bytes.windows(4).any(|part| part == b"\r\n\r\n") {
        if bytes.len() >= 16384 {
            return response(
                &mut stream,
                "431 Request Header Fields Too Large",
                "text/plain",
                b"headers too large",
                false,
            );
        }
        let size = stream.read(&mut chunk)?;
        if size == 0 {
            return Ok(());
        }
        bytes.extend_from_slice(&chunk[..size]);
    }
    let request = std::str::from_utf8(&bytes)?;
    let mut lines = request.split("\r\n");
    let parts = lines.next().unwrap_or("").split_whitespace().collect::<Vec<_>>();
    if parts.len() != 3 || !matches!(parts[2], "HTTP/1.1" | "HTTP/1.0") {
        return response(&mut stream, "400 Bad Request", "text/plain", b"invalid request", false);
    }
    let head = parts[0] == "HEAD";
    if !matches!(parts[0], "GET" | "HEAD") {
        return response(&mut stream, "405 Method Not Allowed", "text/plain", b"GET and HEAD only", head);
    }
    let hosts = lines
        .filter_map(|line| line.split_once(':'))
        .filter(|(name, _)| name.eq_ignore_ascii_case("host"))
        .map(|(_, value)| value.trim())
        .collect::<Vec<_>>();
    let expected = format!("127.0.0.1:{port}");
    let localhost = format!("localhost:{port}");
    if hosts.len() != 1 || !matches!(hosts[0],value if value==expected||value==localhost) {
        return response(&mut stream, "403 Forbidden", "text/plain", b"loopback Host required", head);
    }
    match asset(directory, parts[1]) {
        Ok(asset) => response(&mut stream, "200 OK", asset.mime, &asset.bytes, head),
        Err(_) => response(&mut stream, "404 Not Found", "text/plain", b"not found", head),
    }
}
pub fn run(directory: &Path, port: u16) -> Result<()> {
    let directory = fs::canonicalize(directory)?;
    ensure!(directory.is_dir(), "preview directory required");
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port))?;
    let port = listener.local_addr()?.port();
    println!("Serving {} at http://127.0.0.1:{port}/", directory.display());
    for stream in listener.incoming() {
        let stream = stream?;
        let directory = directory.clone();
        std::thread::spawn(move || {
            if let Err(error) = connection(stream, &directory, port) {
                eprintln!("preview: {error}");
            }
        });
    }
    Ok(())
}
