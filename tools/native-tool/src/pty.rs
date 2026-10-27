use anyhow::{Result, ensure};
use serde_json::{Value, json};
use std::{
    fs::{self, File},
    io::{Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::{fs::PermissionsExt, process::CommandExt},
    },
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};
use unicode_width::UnicodeWidthChar;
struct Screen {
    columns: usize,
    rows: Vec<Vec<char>>,
    x: usize,
    y: usize,
    escape: String,
    pending: Vec<u8>,
    clears: usize,
}
impl Screen {
    fn new() -> Self {
        Self {
            columns: 32,
            rows: vec![vec![' '; 32]; 10],
            x: 0,
            y: 0,
            escape: String::new(),
            pending: vec![],
            clears: 0,
        }
    }
    fn down(&mut self) {
        self.y += 1;
        if self.y == self.rows.len() {
            self.rows.remove(0);
            self.rows.push(vec![' '; self.columns]);
            self.y -= 1;
        }
    }
    fn put(&mut self, c: char) {
        let width = c.width().unwrap_or(0);
        if width == 0 {
            return;
        }
        if self.x + width > self.columns {
            self.x = 0;
            self.down();
        }
        self.rows[self.y][self.x] = c;
        if width == 2 {
            self.rows[self.y][self.x + 1] = '\0';
        }
        self.x += width;
    }
    fn line_at(&self, y: usize) -> String {
        self.rows[y].iter().filter(|c| **c != '\0').collect::<String>().trim_end().to_owned()
    }
    fn line(&self) -> String {
        self.line_at(self.y)
    }
    fn feed(&mut self, bytes: &[u8]) -> Result<()> {
        self.pending.extend_from_slice(bytes);
        let valid = match std::str::from_utf8(&self.pending) {
            Ok(_) => self.pending.len(),
            Err(e) => {
                ensure!(e.error_len().is_none(), "Malformed UTF-8 console output");
                e.valid_up_to()
            }
        };
        let text = std::str::from_utf8(&self.pending[..valid])?.to_owned();
        self.pending.drain(..valid);
        for c in text.chars() {
            if !self.escape.is_empty() {
                self.escape.push(c);
                if self.escape == "\x1b[" {
                    continue;
                }
                if self.escape.chars().count() > 2 && ('@'..='~').contains(&c) {
                    let value = &self.escape[2..self.escape.len() - c.len_utf8()];
                    match c {
                        'K' => self.rows[self.y].fill(' '),
                        'J' => {
                            if value == "2" {
                                self.clears += 1;
                            }
                            self.rows[self.y][self.x.min(self.columns)..].fill(' ');
                            for row in &mut self.rows[self.y + 1..] {
                                row.fill(' ');
                            }
                        }
                        'G' => self.x = value.parse::<usize>().unwrap_or(1) - 1,
                        'A' => self.y = self.y.saturating_sub(value.parse::<usize>().unwrap_or(1)),
                        _ => {}
                    }
                    self.escape.clear();
                }
                continue;
            }
            match c {
                '\x1b' => self.escape.push(c),
                '\r' => self.x = 0,
                '\n' => self.down(),
                _ => self.put(c),
            }
        }
        Ok(())
    }
    fn resize(&mut self, columns: usize) {
        let text = self.line();
        let cursor = self.x;
        self.columns = columns;
        for row in &mut self.rows {
            row.resize(columns, ' ');
        }
        self.rows[self.y].fill(' ');
        self.x = 0;
        let mut cells = 0;
        let mut location = None;
        for c in text.chars() {
            if cells == cursor {
                location = Some((self.x, self.y));
            }
            self.put(c);
            cells += c.width().unwrap_or(0);
        }
        if let Some((x, y)) = location {
            self.x = x;
            self.y = y;
        }
    }
}
struct Pty {
    master: File,
    slave: File,
    child: Child,
    original: libc::termios,
    data: Vec<u8>,
    screen: Screen,
}
fn attributes(file: &File) -> Result<libc::termios> {
    let mut value = std::mem::MaybeUninit::uninit();
    ensure!(unsafe { libc::tcgetattr(file.as_raw_fd(), value.as_mut_ptr()) } == 0, "Cannot inspect PTY");
    Ok(unsafe { value.assume_init() })
}
impl Pty {
    fn new(command: &mut Command) -> Result<Self> {
        let mut master = 0;
        let mut slave = 0;
        let mut size = libc::winsize { ws_row: 10, ws_col: 32, ws_xpixel: 0, ws_ypixel: 0 };
        ensure!(
            unsafe { libc::openpty(&mut master, &mut slave, std::ptr::null_mut(), std::ptr::null_mut(), &mut size) }
                == 0,
            "Cannot open PTY"
        );
        let master = unsafe { File::from_raw_fd(master) };
        let slave = unsafe { File::from_raw_fd(slave) };
        let original = attributes(&slave)?;
        unsafe {
            libc::fcntl(master.as_raw_fd(), libc::F_SETFL, libc::O_NONBLOCK);
            command.pre_exec(|| {
                if libc::setsid() < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        command
            .stdin(Stdio::from(slave.try_clone()?))
            .stdout(Stdio::from(slave.try_clone()?))
            .stderr(Stdio::from(slave.try_clone()?));
        let child = command.spawn()?;
        Ok(Self { master, slave, child, original, data: vec![], screen: Screen::new() })
    }
    fn pump(&mut self, duration: Duration) -> Result<()> {
        let end = Instant::now() + duration;
        loop {
            let mut bytes = [0; 65536];
            match self.master.read(&mut bytes) {
                Ok(0) => {}
                Ok(n) => {
                    self.data.extend_from_slice(&bytes[..n]);
                    self.screen.feed(&bytes[..n])?;
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock || e.raw_os_error() == Some(libc::EIO) => {}
                Err(e) => return Err(e.into()),
            }
            if Instant::now() >= end {
                return Ok(());
            }
            thread::sleep(Duration::from_millis(5));
        }
    }
    fn until(&mut self, mut condition: impl FnMut(&mut Self) -> bool) -> Result<()> {
        let deadline = Instant::now() + Duration::from_secs(12);
        while !condition(self) {
            ensure!(
                Instant::now() < deadline,
                "PTY timeout: {}",
                String::from_utf8_lossy(&self.data[self.data.len().saturating_sub(1500)..])
            );
            self.pump(Duration::from_millis(20))?;
        }
        Ok(())
    }
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.data).into()
    }
    fn send(&mut self, bytes: impl AsRef<[u8]>) -> Result<()> {
        self.master.write_all(bytes.as_ref())?;
        self.pump(Duration::from_millis(120))
    }
    fn finish(&mut self, code: i32) -> Result<()> {
        self.until(|pty| pty.child.try_wait().ok().flatten().is_some())?;
        self.pump(Duration::from_millis(80))?;
        ensure!(self.child.wait()?.code() == Some(code), "Unexpected console exit");
        let mut after = attributes(&self.slave)?;
        let mut before = self.original;
        after.c_lflag &= !libc::PENDIN;
        before.c_lflag &= !libc::PENDIN;
        ensure!(
            after.c_iflag == before.c_iflag
                && after.c_oflag == before.c_oflag
                && after.c_cflag == before.c_cflag
                && after.c_lflag == before.c_lflag
                && after.c_cc == before.c_cc,
            "Terminal attributes not restored"
        );
        Ok(())
    }
}
impl Drop for Pty {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        unsafe {
            libc::tcsetattr(self.slave.as_raw_fd(), libc::TCSANOW, &self.original);
        }
    }
}
fn command(binary: &Path, root: &Path) -> Command {
    let mut c = Command::new(binary);
    crate::isolated(&mut c, root);
    c.env("TERM", "xterm-256color");
    c
}
pub fn setup(binary: &Path) -> Result<()> {
    for terminate in [false, true] {
        let temp = tempfile::tempdir()?;
        let root = temp.path().canonicalize()?;
        let mut cmd = command(binary, &root);
        cmd.args(["setup", "--provider", "deepseek", "--url", "http://127.0.0.1:9", "--model", "offline-fixture"]);
        let mut pty = Pty::new(&mut cmd)?;
        pty.until(|p| p.text().contains("Key storage (env / store)"))?;
        pty.send("store\n")?;
        pty.until(|p| p.text().contains("API key (hidden, stored in OS credential storage):"))?;
        ensure!(attributes(&pty.slave)?.c_lflag & libc::ECHO == 0, "Key input is echoed");
        pty.send("never-echo-this-fake-token")?;
        if terminate {
            unsafe {
                libc::kill(pty.child.id() as i32, libc::SIGTERM);
            }
        } else {
            pty.send(b"\x03")?;
        }
        pty.until(|p| p.child.try_wait().ok().flatten().is_some())?;
        let code = pty.child.wait()?.code().unwrap_or(1);
        ensure!(code != 0, "Cancelled setup succeeded");
        pty.finish(code)?;
        ensure!(
            !pty.text().contains("never-echo-this-fake-token") && !root.join("config/config.json").exists(),
            "Setup leaked or saved a key"
        );
    }
    let temp = tempfile::tempdir()?;
    let root = temp.path().canonicalize()?;
    fs::create_dir(root.join("config"))?;
    let original =
        json!({"protocol":"deepseek","base_url":"https://user:YOUR_PASSWORD@localhost","model":"offline"}).to_string();
    fs::write(root.join("config/config.json"), &original)?;
    let mut cmd = command(binary, &root);
    cmd.args(["setup", "--provider", "deepseek", "--model", "offline", "--key-env", "I_SETUP_FAKE", "--confirm"]);
    let mut pty = Pty::new(&mut cmd)?;
    pty.until(|p| p.child.try_wait().ok().flatten().is_some())?;
    ensure!(!pty.child.wait()?.success() && !pty.text().contains("YOUR_PASSWORD"), "Private URL was exposed");
    ensure!(fs::read_to_string(root.join("config/config.json"))? == original, "Failed setup changed config");
    println!("PASS: setup hidden input, cancellation, signal restoration and private URL");
    Ok(())
}
fn first_launch(binary: &Path, root: &Path, answer: Option<&str>) -> Result<String> {
    let mut cmd = command(binary, root);
    cmd.env("SHELL", "/bin/zsh").env("PATH", "/usr/bin:/bin");
    let mut pty = Pty::new(&mut cmd)?;
    let mut answered = false;
    pty.until(|p| {
        if !answered
            && p.text().contains("Install for this user?")
            && let Some(answer) = answer
        {
            let _ = p.master.write_all(answer.as_bytes());
            answered = true;
        }
        p.text().contains("Provider")
    })?;
    Ok(pty.text())
}
pub fn first_run(binary: &Path) -> Result<()> {
    let temp = tempfile::tempdir()?;
    let root = temp.path().canonicalize()?;
    let mut cmd = command(binary, &root);
    cmd.arg("start");
    let out = crate::run(&mut cmd, b"")?;
    ensure!(
        !out.status.success()
            && !String::from_utf8_lossy(&out.stdout).contains("Install for this user?")
            && !root.join("config/first-run-install.json").exists(),
        "Noninteractive install prompt"
    );
    for accept in [false, true] {
        let temp = tempfile::tempdir()?;
        let root = temp.path().canonicalize()?;
        let text = first_launch(binary, &root, Some(if accept { "y\n" } else { "n\n" }))?;
        ensure!(text.contains("Install for this user?"), "First launch did not offer installation");
        let profile = root.join(".zprofile");
        let before = if accept {
            ensure!(
                text.contains("Installed:") && fs::read(root.join(".local/bin/i"))? == fs::read(binary)?,
                "Installation failed"
            );
            let profile = fs::read_to_string(&profile)?;
            ensure!(profile.contains(root.join(".local/bin").to_str().unwrap()), "Missing PATH update");
            Some(profile)
        } else {
            ensure!(!profile.exists() && !root.join(".local/bin/i").exists(), "Declined installation changed files");
            None
        };
        ensure!(!first_launch(binary, &root, None)?.contains("Install for this user?"), "Repeated installation prompt");
        if let Some(before) = before {
            ensure!(fs::read_to_string(profile)? == before, "Repeated PATH changes");
        }
        ensure!(!root.join("config/config.json").exists(), "Unfinished setup persisted config");
    }
    println!("PASS: first launch, decline once and install without restart");
    Ok(())
}
fn records(root: &Path) -> Vec<String> {
    fs::read_to_string(root.join("calls.jsonl"))
        .unwrap_or_default()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}
fn completed(pty: &Pty) -> usize {
    pty.text().matches("[task finished]").count()
}
fn console_start(binary: &Path, root: &Path, backend: &Path) -> Result<Pty> {
    let mut cmd = command(binary, root);
    cmd.env("I_NATIVE_CONSOLE_FIXTURE", root)
        .args(["--runtime", "console", "--no-logo"])
        .arg(format!("--bin={}", backend.display()))
        .arg(format!("--dir={}", root.display()))
        .arg("--session-name=pty");
    let mut pty = Pty::new(&mut cmd)?;
    pty.until(|p| p.text().contains("\x1b[?2004h") && p.screen.line().contains("lninl>"))?;
    ensure!(attributes(&pty.slave)?.c_lflag & (libc::ECHO | libc::ICANON) == 0, "Console is not raw");
    Ok(pty)
}
pub fn console(binary: &Path) -> Result<()> {
    let temp = tempfile::tempdir()?;
    let root = temp.path().canonicalize()?;
    let backend_path = root.join("backend");
    fs::copy(std::env::current_exe()?, &backend_path)?;
    fs::set_permissions(&backend_path, fs::Permissions::from_mode(0o755))?;
    let mut p = console_start(binary, &root, &backend_path)?;
    for _ in 0..2 {
        p.send("你好")?;
        ensure!(p.screen.line() == "lninl> 你好" && p.screen.x == 11, "Unicode cursor wrong");
        p.send(b"\x7f")?;
        ensure!(p.screen.line() == "lninl> 你" && p.screen.x == 9, "Wide backspace wrong");
        p.send(b"\x7f")?;
        ensure!(p.screen.line() == "lninl>" && p.screen.x == 7, "Prompt cursor wrong");
    }
    p.send("甲丙")?;
    p.send(b"\x1b[D")?;
    p.send("乙")?;
    p.send(b"\x1b[3~\x1b[H\x1b[3~\x1b[F")?;
    p.send("好\r")?;
    p.until(|_| records(&root) == ["乙好"])?;
    p.until(|p| completed(p) == 1)?;
    p.send("\x1b[200~中文\n/exit\n代码\x1b[201~")?;
    ensure!(
        records(&root) == ["乙好"]
            && p.child.try_wait()?.is_none()
            && p.screen.line().ends_with("Content #1 11 chars]"),
        "Bracketed paste executed early"
    );
    p.send("\r")?;
    p.until(|_| records(&root).len() == 2)?;
    ensure!(records(&root)[1] == "中文\n/exit\n代码", "Paste was changed");
    p.until(|p| completed(p) == 2)?;
    p.send("慢任务\r")?;
    p.until(|_| records(&root).len() == 3)?;
    p.send("草稿中文")?;
    p.until(|p| p.text().contains("后台完成"))?;
    p.until(|p| completed(p) == 3)?;
    ensure!(
        p.screen.line() == "lninl> 草稿中文" && !p.text().contains('\u{fffd}') && p.screen.clears == 0,
        "Concurrent output damaged draft or reached terminal escape"
    );
    p.send("\r")?;
    p.until(|_| records(&root).len() == 4)?;
    ensure!(records(&root)[3] == "草稿中文", "Draft changed");
    p.until(|p| completed(p) == 4)?;
    p.send("审批\r")?;
    p.until(|p| p.text().contains("[y/N]"))?;
    p.send("/mode auto\r")?;
    ensure!(!root.join("approval.txt").exists(), "Mode bypassed approval");
    p.send("/mode ask\r")?;
    ensure!(!root.join("approval.txt").exists(), "Mode bypassed approval");
    p.send("y\r")?;
    p.until(|_| root.join("approval.txt").exists())?;
    ensure!(fs::read_to_string(root.join("approval.txt"))? == "y\n", "Approval input wrong");
    p.until(|p| completed(p) == 5)?;
    p.send("未提交")?;
    p.send(b"\x1b[A")?;
    ensure!(p.screen.line().contains("审批"), "History up failed");
    p.send(b"\x1b[B")?;
    ensure!(p.screen.line().contains("未提交"), "History lost draft");
    p.send(b"\x15")?;
    let previous = p.screen.line_at(p.screen.y - 1);
    p.send("很长中文".repeat(40))?;
    ensure!(p.screen.x < 32, "Long line overflow");
    p.screen.resize(20);
    let size = libc::winsize { ws_row: 10, ws_col: 20, ws_xpixel: 0, ws_ypixel: 0 };
    unsafe {
        libc::ioctl(p.slave.as_raw_fd(), libc::TIOCSWINSZ, &size);
    }
    p.pump(Duration::from_millis(300))?;
    ensure!(p.screen.x < 20, "Resize overflow");
    p.send(b"\x15")?;
    ensure!(p.screen.line() == "lninl>" && p.screen.line_at(p.screen.y - 1) == previous, "Resize damaged prior output");
    p.send("/mode auto\r")?;
    p.send("模式\r")?;
    for (n, mode, decision) in [(1, None, "allow"), (2, Some("/mode ask\r"), "ask"), (3, Some("/mode auto\r"), "allow")]
    {
        if let Some(mode) = mode {
            p.send(mode)?;
        }
        let file = root.join(format!("mode-{n}.json"));
        p.until(|_| file.exists())?;
        let value: Value = serde_json::from_slice(&fs::read(file)?)?;
        ensure!(value["decision"] == decision, "Live approval mode failed");
    }
    p.until(|p| completed(p) == 6)?;
    p.send("/mode\r")?;
    p.send("/mode status\r")?;
    ensure!(records(&root).last().is_some_and(|s| s == "模式"), "Mode commands became model tasks");
    p.send("/exit\r")?;
    p.finish(0)?;
    ensure!(p.text().contains("\x1b[?2004l"), "Bracketed paste not disabled");
    drop(p);
    for action in ["ctrl-c", "sigterm", "error"] {
        let missing = root.join("missing-backend");
        let mut p = console_start(binary, &root, if action == "error" { &missing } else { &backend_path })?;
        if action == "error" {
            p.send("测试失败\r")?;
            p.finish(1)?;
        } else {
            p.send("等待\r")?;
            p.until(|p| p.text().contains("WAITING"))?;
            if action == "ctrl-c" {
                p.send(b"\x03")?;
            } else {
                unsafe {
                    libc::kill(p.child.id() as i32, libc::SIGTERM);
                }
            }
            p.finish(130)?;
        }
        ensure!(p.text().contains("\x1b[?2004l"), "Paste mode survived exit");
    }
    println!("PASS: PTY Unicode, editing, paste, history, resize, output, approvals, modes and restoration");
    Ok(())
}
pub fn backend(args: &[String]) -> Result<()> {
    use std::io::BufRead;
    let root = PathBuf::from(std::env::var_os("I_NATIVE_CONSOLE_FIXTURE").unwrap());
    let task = args.last().map(String::as_str).unwrap_or("");
    let mut calls = fs::OpenOptions::new().create(true).append(true).open(root.join("calls.jsonl"))?;
    writeln!(calls, "{}", json!(task))?;
    match task {
        "慢任务" => {
            thread::sleep(Duration::from_millis(300));
            let bytes = "后台你好".as_bytes();
            std::io::stdout().write_all(&bytes[..1])?;
            std::io::stdout().flush()?;
            thread::sleep(Duration::from_millis(200));
            eprint!("\x1b[2J");
            std::io::stdout().write_all(&bytes[1..])?;
            println!();
            thread::sleep(Duration::from_millis(400));
            println!("后台完成");
        }
        "审批" => {
            eprint!("\x1e{}\x1eAllow? [y/N] ", std::env::var("I_APPROVAL_TOKEN")?);
            std::io::stderr().flush()?;
            let mut line = String::new();
            std::io::stdin().read_line(&mut line)?;
            fs::write(root.join("approval.txt"), line)?;
            println!("APPROVED");
        }
        "模式" => {
            let path = &args[args.iter().position(|a| a == "--plugins").unwrap() + 1];
            let manifest: Value = serde_json::from_slice(&fs::read(path)?)?;
            let hook = manifest["hooks"].as_array().unwrap().iter().find(|h| h["module"] == "approval").unwrap();
            let argv: Vec<_> = hook["exec"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
            let mode = argv[argv.iter().position(|a| *a == "--mode-file").unwrap() + 1];
            let mut worker = Command::new(argv[0])
                .args(&argv[1..])
                .current_dir(hook["cwd"].as_str().unwrap())
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()?;
            let mut output = std::io::BufReader::new(worker.stdout.take().unwrap());
            for (i, expected) in ["auto", "ask", "auto"].iter().enumerate() {
                let deadline = Instant::now() + Duration::from_secs(8);
                loop {
                    let value: Value = serde_json::from_slice(&fs::read(mode)?)?;
                    if value["mode"] == *expected {
                        break;
                    }
                    ensure!(Instant::now() < deadline, "Mode update timed out");
                    thread::sleep(Duration::from_millis(10));
                }
                writeln!(
                    worker.stdin.as_mut().unwrap(),
                    "{}",
                    json!({"id":i+1,"event":"tool_approval","workspace":root,"data":{"name":"write","arguments":{},"external":false,"decision":"ask"}})
                )?;
                worker.stdin.as_mut().unwrap().flush()?;
                let mut line = String::new();
                output.read_line(&mut line)?;
                let value: Value = serde_json::from_str(&line)?;
                ensure!(value["id"] == i + 1 && value.get("error").is_none(), "Invalid approval frame");
                fs::write(root.join(format!("mode-{}.json", i + 1)), value["data"].to_string())?;
            }
            drop(worker.stdin.take());
            worker.wait()?;
            println!("MODE DONE");
        }
        "等待" => {
            println!("WAITING");
            std::io::stdout().flush()?;
            thread::sleep(Duration::from_secs(60));
        }
        _ => println!("任务完成"),
    };
    Ok(())
}
