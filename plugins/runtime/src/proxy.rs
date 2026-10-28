//! Host-owned session routing; never mutate process environment while threads run.
use super::{paths::AppPaths, state};
use anyhow::{Result, bail, ensure};
use serde_json::{Value, json};
use std::process::Command;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) enum Setting {
    #[default]
    Inherit,
    Direct,
    Url(String),
}
const KEYS: [&str; 6] = ["HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY", "http_proxy", "https_proxy", "all_proxy"];
const SCOPES: [&str; 5] = ["default", "chatgpt", "claude", "plugins", "model"];
fn read(paths: &AppPaths) -> Result<Value> {
    let file = paths.config_dir.join("proxy.json");
    if !file.try_exists()? {
        return Ok(json!({}));
    }
    let value: Value = serde_json::from_slice(&state::read_bounded(&file, 8192)?)?;
    ensure!(
        value.as_object().is_some_and(|obj| obj.keys().all(|key| SCOPES.contains(&key.as_str()))),
        "Invalid proxy configuration"
    );
    for value in value.as_object().unwrap().values() {
        Setting::parse(value.as_str().ok_or_else(|| anyhow::anyhow!("Invalid proxy setting"))?)?;
    }
    Ok(value)
}
fn resolve(value: &Value, scope: &str) -> Result<Setting> {
    match value.get(scope).or_else(|| value.get("default")).and_then(Value::as_str) {
        Some(value) => Setting::parse(value),
        None => Ok(Setting::Inherit),
    }
}
pub(super) fn snapshot(scope: &str) -> Result<Setting> {
    resolve(&read(&AppPaths::desktop()?)?, scope)
}
impl Setting {
    fn parse(input: &str) -> Result<Self> {
        const INVALID: &str = "Invalid proxy. Use no, noproxy, env, PORT, :PORT, HOST:PORT or an http/https/socks5/socks5h URL (no credentials, path, query or fragment).";
        let input = input.trim();
        match input.to_ascii_lowercase().as_str() {
            "no" | "noproxy" => return Ok(Self::Direct),
            "env" => return Ok(Self::Inherit),
            _ => {}
        }
        let expanded = if input.strip_prefix(':').unwrap_or(input).bytes().all(|b| b.is_ascii_digit()) {
            let port: u16 = input.trim_start_matches(':').parse().map_err(|_| anyhow::anyhow!(INVALID))?;
            ensure!(port != 0, INVALID);
            format!("http://127.0.0.1:{port}")
        } else if input.contains("://") {
            input.to_owned()
        } else {
            format!("http://{input}")
        };
        let url = url::Url::parse(&expanded).map_err(|_| anyhow::anyhow!(INVALID))?;
        ensure!(
            matches!(url.scheme(), "http" | "https" | "socks5" | "socks5h")
                && url.host_str().is_some()
                && url.port_or_known_default().is_some_and(|port| port > 0)
                && url.username().is_empty()
                && url.password().is_none()
                && matches!(url.path(), "" | "/")
                && url.query().is_none()
                && url.fragment().is_none(),
            INVALID
        );
        Ok(Self::Url(url.as_str().trim_end_matches('/').to_owned()))
    }
    pub fn apply(&self, command: &mut Command) {
        if matches!(self, Self::Inherit) {
            return;
        }
        for key in KEYS {
            command.env_remove(key);
        }
        let (value, bypass) = match self {
            Self::Direct => ("", "*"),
            Self::Url(value) => (value.as_str(), "localhost,127.0.0.1,::1"),
            Self::Inherit => unreachable!(),
        };
        command.env("NO_PROXY", bypass).env("no_proxy", bypass);
        if !value.is_empty() {
            for key in KEYS {
                command.env(key, value);
            }
        }
    }
    pub fn download_proxy(&self) -> Result<Option<ureq::Proxy>> {
        Ok(match self {
            Self::Inherit => ureq::Proxy::try_from_env(),
            Self::Direct => None,
            Self::Url(url) => Some(ureq::Proxy::new(url)?),
        })
    }
    fn label(&self) -> &str {
        match self {
            Self::Inherit => "environment/configuration defaults",
            Self::Direct => "direct (proxy disabled)",
            Self::Url(url) => url,
        }
    }
}
pub(super) fn handle(line: &str, paths: &AppPaths) -> Result<Option<String>> {
    let args: Vec<_> = line.split_whitespace().collect();
    if args.first() != Some(&"/proxy") {
        return Ok(None);
    }
    match args.as_slice() {
        [_] => {
            let value = read(paths)?;
            Ok(Some(
                SCOPES
                    .iter()
                    .map(|scope| {
                        Ok(format!(
                            "Proxy {scope}: {}{}",
                            resolve(&value, scope)?.label(),
                            if *scope != "default" && value.get(scope).is_none() { " (follows default)" } else { "" }
                        ))
                    })
                    .collect::<Result<Vec<_>>>()?
                    .join("\n"),
            ))
        }
        [_, value] => update(paths, "default", value).map(Some),
        [_, scope, value] if SCOPES.contains(scope) => update(paths, scope, value).map(Some),
        _ => bail!(
            "Usage: /proxy [default|chatgpt|claude|plugins|model] [no|noproxy|env|default|PORT|:PORT|HOST:PORT|URL]"
        ),
    }
}
fn update(paths: &AppPaths, scope: &str, input: &str) -> Result<String> {
    let setting = if input == "default" { None } else { Some(Setting::parse(input)?) };
    std::fs::create_dir_all(&paths.config_dir)?;
    let _guard = state::Lock::acquire(&paths.config_dir.join("proxy.lock"))?;
    let mut value = read(paths)?;
    if let Some(setting) = setting {
        value[scope] = json!(match setting {
            Setting::Inherit => "env".into(),
            Setting::Direct => "no".into(),
            Setting::Url(url) => url,
        });
    } else {
        value.as_object_mut().unwrap().remove(scope);
    }
    state::atomic(&paths.config_dir.join("proxy.json"), &value)?;
    Ok(format!(
        "Proxy {scope}: {}. Saved for this user; subsequent requests use this setting, running requests are unchanged.",
        resolve(&value, scope)?.label()
    ))
}
pub(crate) fn model() -> Result<Option<String>> {
    Ok(match snapshot("model")? {
        Setting::Inherit => None,
        Setting::Direct => Some(String::new()),
        Setting::Url(url) => Some(url),
    })
}
pub(crate) fn apply_connection(command: &mut Command, id: &str) -> Result<()> {
    ensure!(matches!(id, "chatgpt" | "claude"), "Unknown managed connection proxy scope");
    // Managed workers normally clear their environment; restore only routing variables.
    for key in KEYS.into_iter().chain(["NO_PROXY", "no_proxy"]) {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
    }
    snapshot(id)?.apply(command);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn user_rules_persist_and_specific_overrides_can_be_reset() {
        let root = tempfile::tempdir().unwrap();
        let paths = AppPaths {
            config_dir: root.path().join("config"),
            data_dir: root.path().join("data"),
            cache_dir: root.path().join("cache"),
            runtime_dir: root.path().join("runtime"),
        };
        handle("/proxy 3067", &paths).unwrap();
        handle("/proxy claude no", &paths).unwrap();
        handle("/proxy plugins noproxy", &paths).unwrap();
        handle("/proxy model socks5://127.0.0.1:3067", &paths).unwrap();
        let saved = read(&paths).unwrap();
        assert_eq!(resolve(&saved, "chatgpt").unwrap(), Setting::Url("http://127.0.0.1:3067".into()));
        assert_eq!(resolve(&saved, "claude").unwrap(), Setting::Direct);
        assert_eq!(resolve(&saved, "plugins").unwrap(), Setting::Direct);
        assert_eq!(resolve(&saved, "model").unwrap(), Setting::Url("socks5://127.0.0.1:3067".into()));
        assert!(handle("/proxy claude http://user:YOUR_PASSWORD@localhost:1", &paths).is_err());
        assert_eq!(read(&paths).unwrap(), saved);
        handle("/proxy claude default", &paths).unwrap();
        assert_eq!(resolve(&read(&paths).unwrap(), "claude").unwrap(), resolve(&saved, "default").unwrap());
        handle("/proxy no", &paths).unwrap();
        assert_eq!(resolve(&read(&paths).unwrap(), "chatgpt").unwrap(), Setting::Direct);
        handle("/proxy env", &paths).unwrap();
        assert_eq!(resolve(&read(&paths).unwrap(), "chatgpt").unwrap(), Setting::Inherit);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(paths.config_dir.join("proxy.json")).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }
    #[test]
    fn direct_download_disables_environment_proxy_and_socks_is_supported() {
        assert!(Setting::Direct.download_proxy().unwrap().is_none());
        let proxy = Setting::parse("socks5://127.0.0.1:3067").unwrap().download_proxy().unwrap().unwrap();
        assert_eq!(proxy.protocol(), ureq::ProxyProtocol::Socks5);
    }
    #[test]
    fn accepts_shortcuts_and_explicit_protocols() {
        for input in ["3067", ":3067", "127.0.0.1:3067", "http://127.0.0.1:3067"] {
            assert_eq!(Setting::parse(input).unwrap(), Setting::Url("http://127.0.0.1:3067".into()));
        }
        for input in ["no", "noproxy", "NO"] {
            assert_eq!(Setting::parse(input).unwrap(), Setting::Direct);
        }
        for scheme in ["https", "socks5", "socks5h"] {
            let input = format!("{scheme}://127.0.0.1:3067");
            assert_eq!(Setting::parse(&input).unwrap(), Setting::Url(input));
        }
        assert_eq!(Setting::parse("env").unwrap(), Setting::Inherit);
    }
    #[test]
    fn rejects_invalid_input_without_echoing_secrets() {
        for input in [
            "0",
            "65536",
            ":",
            "ftp://localhost:1",
            "http://",
            "http://a:bad",
            "http://a/path",
            "http://a?key=PRIVATE",
            "http://user:YOUR_PASSWORD@localhost:3067",
        ] {
            let error = Setting::parse(input).unwrap_err().to_string();
            assert!(!error.contains("PRIVATE"));
            assert!(!error.contains("YOUR_PASSWORD"));
        }
    }
    #[test]
    fn command_settings_override_both_cases_and_preserve_parent_environment() {
        let mut command = std::process::Command::new("fixture");
        Setting::Direct.apply(&mut command);
        let env: std::collections::HashMap<_, _> =
            command.get_envs().map(|(k, v)| (k.to_str().unwrap(), v.and_then(|v| v.to_str()))).collect();
        assert_eq!(env["NO_PROXY"], Some("*"));
        assert_eq!(env["https_proxy"], None);
        assert_eq!(env["ALL_PROXY"], None);
        let mut command = std::process::Command::new("fixture");
        Setting::parse("3067").unwrap().apply(&mut command);
        let env: std::collections::HashMap<_, _> =
            command.get_envs().map(|(k, v)| (k.to_str().unwrap(), v.and_then(|v| v.to_str()))).collect();
        assert_eq!(env["HTTPS_PROXY"], Some("http://127.0.0.1:3067"));
        assert_eq!(env["NO_PROXY"], Some("localhost,127.0.0.1,::1"));
    }
}
