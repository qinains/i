use super::*;
pub(super) struct Worker {
    child: GroupChild,
    send: mpsc::Sender<Value>,
    receive: mpsc::Receiver<Result<Value>>,
    serial: u64,
}
impl Worker {
    #[cfg(test)]
    pub(super) fn start(exec: &Value, dir: &Path, limit: usize) -> Result<Self> {
        Self::start_routed(exec, dir, limit, None)
    }
    pub(super) fn start_routed(exec: &Value, dir: &Path, limit: usize, route: Option<&str>) -> Result<Self> {
        let argv: Vec<String> = serde_json::from_value(exec.clone()).context("exec must be string array")?;
        let program = argv.first().context("empty exec")?;
        let program = if Path::new(program).components().count() > 1 { dir.join(program) } else { program.into() };
        let mut command = Command::new(&program);
        command
            .args(&argv[1..])
            .current_dir(dir)
            .env_clear()
            .env("PATH", env::var_os("PATH").unwrap_or_default())
            .env("PYTHONUTF8", "1")
            .envs(
                "SystemRoot TEMP TMP HOME USERPROFILE I_CONFIG_DIR I_DATA_DIR I_CACHE_DIR I_RUNTIME_DIR"
                    .split_whitespace()
                    .filter_map(|key| env::var_os(key).map(|value| (key, value))),
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(feature = "bundled")]
        if let Some(route) = route {
            i_plugins::apply_connection_proxy(&mut command, route)?;
        }
        #[cfg(not(feature = "bundled"))]
        let _ = route;
        let mut child =
            command.group_spawn().with_context(|| format!("cannot start plugin executable {}", program.display()))?;
        let mut input = child.inner().stdin.take().context("missing stdin")?;
        let mut output = io::BufReader::new(child.inner().stdout.take().context("missing stdout")?);
        let error = child.inner().stderr.take().context("missing stderr")?;
        let (send, requests) = mpsc::channel::<Value>();
        let (results, receive) = mpsc::channel();
        let errors = results.clone();
        thread::spawn(move || {
            let mut bytes = Vec::new();
            if error.take(limit as u64 + 1).read_to_end(&mut bytes).is_err() || bytes.len() > limit {
                let _ = errors.send(Err(anyhow::anyhow!("worker stderr limit/read failure")));
            }
        });
        thread::spawn(move || {
            for request in requests {
                let result = (|| -> Result<Value> {
                    let bytes = serde_json::to_vec(&request)?;
                    ensure!(bytes.len() < 48 * 1024 * 1024, "worker input limit");
                    input.write_all(&bytes)?;
                    input.write_all(b"\n")?;
                    input.flush()?;
                    let mut bytes = Vec::new();
                    output.by_ref().take(48 * 1024 * 1024 + 1).read_until(b'\n', &mut bytes)?;
                    ensure!(bytes.len() <= 48 * 1024 * 1024 && bytes.last() == Some(&b'\n'), "worker EOF/frame limit");
                    serde_json::from_slice(&bytes).context("invalid worker JSON")
                })();
                if results.send(result).is_err() {
                    break;
                }
            }
        });
        Ok(Self { child, send, receive, serial: 0 })
    }
    pub(super) fn call(&mut self, mut request: Value, timeout: Duration) -> Result<Value> {
        ensure!(self.child.try_wait()?.is_none(), "worker exited; request not sent");
        ensure!(self.receive.try_recv().is_err(), "unsolicited worker output/error");
        self.serial += 1;
        request["id"] = json!(self.serial);
        self.send.send(request)?;
        let mut reply = self.receive.recv_timeout(timeout).context("worker timeout/disconnected")??;
        ensure!(reply["id"] == self.serial, "worker response ID mismatch");
        ensure!(reply.get("error").is_none(), "worker: {}", clip(&reply["error"].to_string(), 1000));
        ensure!(reply["data"].is_object(), "worker data must be an object");
        Ok(reply["data"].take())
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
