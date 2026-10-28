use super::*;

#[test]
fn user_model_proxy_routes_a_real_request_and_overrides_old_model_config() {
    let root = tempfile::tempdir().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let proxy = format!("http://{}", listener.local_addr().unwrap());
    let config = root.path().join(".app/config");
    fs::create_dir_all(&config).unwrap();
    fs::write(config.join("proxy.json"), json!({"default":"no","model":proxy}).to_string()).unwrap();
    let task = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut socket = loop {
            match listener.accept() {
                Ok((socket, _)) => break socket,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(Instant::now() < deadline, "request did not reach selected proxy");
                    thread::sleep(Duration::from_millis(10));
                }
                Err(error) => panic!("{error}"),
            }
        };
        socket.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let mut header = Vec::new();
        let mut byte = [0];
        while !header.ends_with(b"\r\n\r\n") {
            socket.read_exact(&mut byte).unwrap();
            header.push(byte[0]);
        }
        let header = String::from_utf8(header).unwrap();
        assert!(header.starts_with("POST http://model.invalid/v1/chat/completions HTTP/1.1"), "{header}");
        let length: usize = header
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length").then(|| value.trim().parse().unwrap())
            })
            .unwrap();
        socket.read_exact(&mut vec![0; length]).unwrap();
        let body = final_reply().to_string();
        write!(socket,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
    });
    let model_config = config.join("old-model.json");
    fs::write(&model_config, json!({"proxy":"http://127.0.0.1:1"}).to_string()).unwrap();
    let output = run(root.path(), "http://model.invalid/v1", &["--config", model_config.to_str().unwrap(), "hello"]);
    task.join().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
}
