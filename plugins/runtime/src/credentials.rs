//! OS credential storage. Called in the host, never through provider RPC.
use super::*;
#[cfg(any(target_os = "macos", target_os = "linux"))]
const SERVICE: &str = "org.lninl.i";
fn validate(name: &str) -> Result<()> {
    let id = name.strip_prefix("I_CREDENTIAL_").context("invalid credential reference")?;
    ensure!(id.len() == 32 && id.bytes().all(|b| b.is_ascii_hexdigit()), "invalid credential reference");
    Ok(())
}
pub(crate) fn store(key: &str) -> Result<String> {
    ensure!(
        !key.is_empty() && key.len() <= 8192 && !key.chars().any(char::is_control),
        "invalid API key length/control characters"
    );
    let name = format!("I_CREDENTIAL_{}", uuid::Uuid::new_v4().simple());
    platform::store(&name, key)?;
    Ok(name)
}
pub(crate) fn load(name: &str) -> Result<String> {
    validate(name)?;
    let key = platform::load(name)?;
    ensure!(!key.is_empty() && key.len() <= 8192 && !key.chars().any(char::is_control), "invalid stored credential");
    Ok(key)
}
pub(crate) fn remove(name: &str) -> Result<()> {
    validate(name)?;
    platform::remove(name)
}
#[cfg(target_os = "macos")]
mod platform {
    use super::*;
    use std::{ffi::c_void, ptr};
    #[link(name = "Security", kind = "framework")]
    unsafe extern "C" {
        fn SecKeychainAddGenericPassword(
            keychain: *const c_void,
            service_len: u32,
            service: *const u8,
            account_len: u32,
            account: *const u8,
            password_len: u32,
            password: *const u8,
            item: *mut *mut c_void,
        ) -> i32;
        fn SecKeychainFindGenericPassword(
            keychain: *const c_void,
            service_len: u32,
            service: *const u8,
            account_len: u32,
            account: *const u8,
            password_len: *mut u32,
            password: *mut *mut c_void,
            item: *mut *mut c_void,
        ) -> i32;
        fn SecKeychainItemFreeContent(attributes: *mut c_void, data: *mut c_void) -> i32;
        fn SecKeychainItemDelete(item: *const c_void) -> i32;
    }
    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFRelease(value: *const c_void);
    }
    pub(super) fn store(name: &str, key: &str) -> Result<()> {
        // Bounded byte strings stay alive for the synchronous system API call.
        let status = unsafe {
            SecKeychainAddGenericPassword(
                ptr::null(),
                SERVICE.len() as u32,
                SERVICE.as_ptr(),
                name.len() as u32,
                name.as_ptr(),
                key.len() as u32,
                key.as_ptr(),
                ptr::null_mut(),
            )
        };
        ensure!(status == 0, "macOS Keychain store failed ({status}); use an environment variable if unavailable");
        Ok(())
    }
    pub(super) fn load(name: &str) -> Result<String> {
        let (mut len, mut bytes) = (0, ptr::null_mut());
        let status = unsafe {
            SecKeychainFindGenericPassword(
                ptr::null(),
                SERVICE.len() as u32,
                SERVICE.as_ptr(),
                name.len() as u32,
                name.as_ptr(),
                &mut len,
                &mut bytes,
                ptr::null_mut(),
            )
        };
        ensure!(status == 0, "macOS Keychain lookup failed ({status}); unlock Keychain or rerun setup");
        let result = if len > 0 && len <= 8192 && !bytes.is_null() {
            String::from_utf8(unsafe { std::slice::from_raw_parts(bytes.cast::<u8>(), len as usize) }.to_vec())
                .context("invalid credential encoding")
        } else {
            bail_result()
        };
        unsafe {
            SecKeychainItemFreeContent(ptr::null_mut(), bytes);
        }
        result
    }
    fn bail_result() -> Result<String> {
        bail!("invalid stored credential length")
    }
    pub(super) fn remove(name: &str) -> Result<()> {
        let mut item = ptr::null_mut();
        let status = unsafe {
            SecKeychainFindGenericPassword(
                ptr::null(),
                SERVICE.len() as u32,
                SERVICE.as_ptr(),
                name.len() as u32,
                name.as_ptr(),
                ptr::null_mut(),
                ptr::null_mut(),
                &mut item,
            )
        };
        ensure!(status == 0, "Keychain rollback lookup failed ({status})");
        let status = unsafe {
            let status = SecKeychainItemDelete(item);
            CFRelease(item);
            status
        };
        ensure!(status == 0, "Keychain rollback failed ({status})");
        Ok(())
    }
}
#[cfg(any(target_os = "linux", windows))]
fn command(program: &str, args: &[&str], input: Vec<u8>) -> Result<Vec<u8>> {
    use std::{
        process::{Command, Stdio},
        time::Duration,
    };
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .context("credential helper unavailable; use an environment variable")?;
    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let result = (|| -> Result<Vec<u8>> {
            stdin.write_all(&input)?;
            drop(stdin);
            let mut output = Vec::new();
            stdout.take(16385).read_to_end(&mut output)?;
            ensure!(output.len() <= 16384, "credential helper output exceeds limit");
            Ok(output)
        })();
        let _ = tx.send(result);
    });
    let result = rx.recv_timeout(Duration::from_secs(30));
    if result.is_err() {
        let _ = child.kill();
        let _ = child.wait();
        bail!("credential helper timed out");
    }
    let output = result.unwrap();
    if output.is_err() {
        let _ = child.kill();
    }
    loop {
        if let Some(status) = child.try_wait()? {
            ensure!(status.success(), "credential helper failed; unlock OS storage or use environment variable");
            return output;
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            bail!("credential helper timed out");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
#[cfg(target_os = "linux")]
mod platform {
    use super::*;
    pub(super) fn store(name: &str, key: &str) -> Result<()> {
        command(
            "secret-tool",
            &["store", "--label=i API credential", "service", SERVICE, "account", name],
            key.as_bytes().to_vec(),
        )?;
        Ok(())
    }
    pub(super) fn load(name: &str) -> Result<String> {
        Ok(String::from_utf8(command("secret-tool", &["lookup", "service", SERVICE, "account", name], vec![])?)?
            .trim_end_matches(['\r', '\n'])
            .to_owned())
    }
    pub(super) fn remove(name: &str) -> Result<()> {
        command("secret-tool", &["clear", "service", SERVICE, "account", name], vec![])?;
        Ok(())
    }
}
#[cfg(windows)]
mod platform {
    use super::*;
    fn path(name: &str) -> Result<PathBuf> {
        Ok(paths::AppPaths::desktop()?.config_dir.join("credentials").join(format!("{name}.json")))
    }
    pub(super) fn store(name: &str, key: &str) -> Result<()> {
        let script = "$ErrorActionPreference='Stop'; [Console]::InputEncoding=[Text.UTF8Encoding]::new($false); [Console]::OutputEncoding=[Text.UTF8Encoding]::new($false); $v=[Console]::In.ReadToEnd(); $s=ConvertTo-SecureString -String $v -AsPlainText -Force; [Console]::Write((ConvertFrom-SecureString $s))";
        let encrypted = String::from_utf8(command(
            "powershell.exe",
            &["-NoProfile", "-NonInteractive", "-Command", script],
            key.as_bytes().to_vec(),
        )?)?;
        let file = path(name)?;
        std::fs::create_dir_all(file.parent().unwrap())?;
        state::atomic(&file, &json!(encrypted))
    }
    pub(super) fn load(name: &str) -> Result<String> {
        let encrypted: String = serde_json::from_slice(&state::read_bounded(&path(name)?, 16384)?)?;
        let script = "$ErrorActionPreference='Stop'; [Console]::InputEncoding=[Text.UTF8Encoding]::new($false); [Console]::OutputEncoding=[Text.UTF8Encoding]::new($false); $s=ConvertTo-SecureString ([Console]::In.ReadToEnd()); $b=[Runtime.InteropServices.Marshal]::SecureStringToBSTR($s); try {[Console]::Write([Runtime.InteropServices.Marshal]::PtrToStringBSTR($b))} finally {[Runtime.InteropServices.Marshal]::ZeroFreeBSTR($b)}";
        Ok(String::from_utf8(command(
            "powershell.exe",
            &["-NoProfile", "-NonInteractive", "-Command", script],
            encrypted.into_bytes(),
        )?)?)
    }
    pub(super) fn remove(name: &str) -> Result<()> {
        std::fs::remove_file(path(name)?)?;
        Ok(())
    }
}
#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
mod platform {
    use super::*;
    pub(super) fn store(_: &str, _: &str) -> Result<()> {
        bail!("OS credential storage unavailable; use environment variable")
    }
    pub(super) fn load(_: &str) -> Result<String> {
        bail!("OS credential storage unavailable")
    }
    pub(super) fn remove(_: &str) -> Result<()> {
        bail!("OS credential storage unavailable")
    }
}
