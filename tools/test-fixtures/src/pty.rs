#[cfg(unix)]
pub fn run(binary: &str) {
    use std::{
        fs::File,
        io::Write,
        os::fd::{AsRawFd, FromRawFd},
        path::Path,
        process::{Command, Stdio},
        thread,
        time::{Duration, Instant},
    };

    struct Console {
        master: File,
        child: std::process::Child,
    }
    impl Drop for Console {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
    impl Console {
        fn read(&mut self, duration: Duration) -> Vec<u8> {
            let end = Instant::now() + duration;
            let mut output = Vec::new();
            while Instant::now() < end {
                let mut descriptor = libc::pollfd { fd: self.master.as_raw_fd(), events: libc::POLLIN, revents: 0 };
                let remaining = end.saturating_duration_since(Instant::now());
                // The descriptor remains owned by self throughout poll/read.
                if unsafe { libc::poll(&mut descriptor, 1, remaining.as_millis().max(1) as i32) } <= 0 {
                    break;
                }
                let mut bytes = [0u8; 65536];
                let count = unsafe { libc::read(descriptor.fd, bytes.as_mut_ptr().cast(), bytes.len()) };
                if count <= 0 {
                    break;
                }
                output.extend_from_slice(&bytes[..count as usize]);
            }
            output
        }
        fn send(&mut self, bytes: &[u8]) -> String {
            self.master.write_all(bytes).unwrap();
            String::from_utf8_lossy(&self.read(Duration::from_millis(250))).into_owned()
        }
        fn expect(&mut self, bytes: &[u8], ready: impl Fn(&str) -> bool) -> String {
            self.master.write_all(bytes).unwrap();
            let deadline = Instant::now() + Duration::from_secs(5);
            let mut output = Vec::new();
            loop {
                output.extend(self.read(Duration::from_millis(250)));
                let text = String::from_utf8_lossy(&output);
                if ready(&text) {
                    return text.into_owned();
                }
                assert!(Instant::now() < deadline, "console response timed out: {text}");
            }
        }
    }
    let binary = Path::new(binary).canonicalize().expect("build i-plugins before the PTY test");
    let root = tempfile::tempdir().unwrap();
    let mut master = -1;
    let mut slave = -1;
    let mut size = libc::winsize { ws_row: 30, ws_col: 110, ws_xpixel: 0, ws_ypixel: 0 };
    // openpty initializes two new owned descriptors on success.
    assert_eq!(
        unsafe { libc::openpty(&mut master, &mut slave, std::ptr::null_mut(), std::ptr::null_mut(), &raw mut size) },
        0
    );
    let master = unsafe { File::from_raw_fd(master) };
    let slave = unsafe { File::from_raw_fd(slave) };
    let mut command = Command::new(binary);
    command
        .args(["console", "--no-logo", "--bin", "/usr/bin/false", "-C"])
        .arg(root.path())
        .current_dir(root.path())
        .env("HOME", root.path())
        .env("TERM", "xterm-256color")
        .env("NO_COLOR", "1")
        .stdin(Stdio::from(slave.try_clone().unwrap()))
        .stdout(Stdio::from(slave.try_clone().unwrap()))
        .stderr(Stdio::from(slave));
    for (key, name) in
        [("I_CONFIG_DIR", "config"), ("I_DATA_DIR", "data"), ("I_CACHE_DIR", "cache"), ("I_RUNTIME_DIR", "runtime")]
    {
        command.env(key, root.path().join(name));
    }
    let child = command.spawn().unwrap();
    let mut console = Console { master, child };
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut startup = Vec::new();
    while !(startup.windows(b"\x1b[?2004h".len()).any(|bytes| bytes == b"\x1b[?2004h")
        && startup.windows(b"lninl>".len()).any(|bytes| bytes == b"lninl>"))
    {
        assert!(Instant::now() < deadline, "console did not become ready: {}", String::from_utf8_lossy(&startup));
        startup.extend(console.read(Duration::from_millis(25)));
    }
    let menu = console.expect(b"/", |text| text.contains("Show available commands") && text.contains("/mode"));
    assert!(menu.contains("Show available commands") && menu.contains("/mode"), "{menu}");
    let filtered = console.expect(b"mo", |text| text.contains("/mode"));
    assert!(filtered.contains("/mode") && !filtered.contains("Show available commands"), "{filtered}");
    let filled = console.expect(b"\r", |text| text.contains("/mode ask"));
    assert!(filled.contains("/mode ask") && !filled.contains("[mode:"), "{filled}");
    let chosen = console.expect(b"\x1b[B\t", |text| text.contains("/mode auto"));
    assert!(chosen.contains("/mode auto") && !chosen.contains("[mode:"), "{chosen}");
    assert!(console.expect(b"\r", |text| text.contains("[mode: auto;")).contains("[mode: auto;"));
    let pasted = console.send(b"\x1b[200~/exit\n/mode ask\x1b[201~");
    assert!(console.child.try_wait().unwrap().is_none() && !pasted.contains("exit requested"));
    assert!(!pasted.contains("[mode: ask;"));
    let rejected = console.expect(b"\r", |text| {
        text.contains("invalid slash command arguments") || text.contains("unknown or multiline slash command")
    });
    assert!(
        rejected.contains("invalid slash command arguments") || rejected.contains("unknown or multiline slash command"),
        "{rejected}"
    );
    assert!(
        console
            .expect(b"/not-real\r", |text| text.contains("unknown or multiline slash command"))
            .contains("unknown or multiline slash command")
    );
    console.send(b"/help");
    console.send(b"\x1b");
    thread::sleep(Duration::from_millis(150));
    console.read(Duration::from_millis(250));
    assert!(console.expect(b"\r", |text| text.contains("Show available commands")).contains("Show available commands"));
    console.send(b"/ex\t");
    assert!(console.child.try_wait().unwrap().is_none());
    console.send(b"\r");
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(status) = console.child.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        assert!(Instant::now() < deadline, "console exit timed out");
        thread::sleep(Duration::from_millis(10));
    }
    println!("PASS: slash menu, prefix, arrows, fill-only, explicit execution, Escape, unknown command, paste, exit");
}

#[cfg(not(unix))]
pub fn run(_: &str) {
    panic!("the PTY smoke test requires Unix");
}
