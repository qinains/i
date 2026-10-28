use crate::{backend::Backend, bridge};
use anyhow::{anyhow, bail, Result};
use clap::{Parser, Subcommand};
use serde_json::{json, Value};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};

#[derive(Parser)]
#[command(about = "Native OAuth connection plugin for i")]
struct Args {
    #[arg(long, global = true)]
    data_dir: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Sign in using your local system browser.
    Login {
        #[arg(long)]
        account: Option<String>,
    },
    Status,
    Models,
    Use {
        id: String,
    },
    Logout {
        #[arg(long)]
        account: Option<String>,
    },
    /// Write an i configuration using this native plugin, without credentials.
    Configure {
        #[arg(long)]
        model: String,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Start the origin-bound local browser assistant connection.
    Serve {
        #[arg(long, default_value = "https://i.lninl.com")]
        origin: String,
        #[arg(long, default_value_t = 0)]
        port: u16,
    },
    #[command(hide = true)]
    Worker,
    Manifest,
}

pub async fn run(claude: bool) -> Result<()> {
    let mut args = Args::parse();
    let backend = match &args.data_dir {
        Some(path) => Backend::at(claude, path.clone())?,
        None => {
            if claude {
                Backend::claude()?
            } else {
                Backend::chatgpt()?
            }
        }
    };
    let default_directory = match &args.data_dir {
        Some(path) => path.clone(),
        None => dirs_next::config_dir()
            .ok_or_else(|| anyhow!("No application configuration directory"))?
            .join("i")
            .join(if claude { "claude" } else { "chatgpt" }),
    };
    args.data_dir = Some(resolve_data_directory(args.data_dir.as_deref(), &default_directory)?);
    let value = match args.command {
        Command::Login { account } => {
            if claude {
                eprintln!("Claude Console API billing applies. OAuth avoids manual API-key entry; Claude Pro/Max plan usage is not supported.");
            }
            backend.login(account.as_deref()).await?
        }
        Command::Status => backend.status().await?,
        Command::Models => backend.models().await?,
        Command::Use { id } => backend.select(&id).await?,
        Command::Logout { account } => backend.logout(account.as_deref()).await?,
        Command::Serve { origin, port } => return bridge::serve(backend, origin, port).await,
        Command::Worker => return worker(backend).await,
        Command::Manifest => backend.manifest(),
        Command::Configure { model, output } => {
            if model.trim().is_empty() || model.len() > 256 {
                bail!("Choose a valid model with --model");
            }
            let value = config_value_with_directory(
                claude,
                &model,
                &std::env::current_exe().map_err(|_| anyhow!("Cannot locate connection plugin"))?,
                args.data_dir.as_deref(),
            )?;
            let output = output.unwrap_or_else(|| {
                args.data_dir
                    .clone()
                    .unwrap_or_else(|| {
                        dirs_next::config_dir().unwrap_or_else(|| PathBuf::from(".")).join("i").join(if claude {
                            "claude"
                        } else {
                            "chatgpt"
                        })
                    })
                    .join(if claude { "claude.json" } else { "chatgpt.json" })
            });
            write_config(&output, &value)?;
            json!({"configured":true,"path":output,"billing":if claude {"Claude Console API; not Claude Pro/Max subscription"} else {"ChatGPT authorized plan usage"}})
        }
    };
    println!("{}", serde_json::to_string_pretty(&value).map_err(|_| anyhow!("Cannot serialize connection result"))?);
    Ok(())
}

fn config_value(claude: bool, model: &str, exe: &Path) -> Value {
    json!({"protocol":if claude {"anthropic"} else {"responses"},"base_url":if claude {"https://api.anthropic.com/v1"} else {"https://api.openai.com/v1"},"model":model,"max_tokens":8192,"provider":{"exec":[exe,"worker"],"completion":"plugin"}})
}

fn resolve_data_directory(explicit: Option<&Path>, default: &Path) -> Result<PathBuf> {
    explicit.unwrap_or(default).canonicalize().map_err(|_| anyhow!("Cannot resolve plugin data directory"))
}

fn config_value_with_directory(claude: bool, model: &str, exe: &Path, directory: Option<&Path>) -> Result<Value> {
    let mut value = config_value(claude, model, exe);
    if let Some(directory) = directory {
        let canonical = directory.canonicalize().map_err(|_| anyhow!("Cannot resolve plugin data directory"))?;
        value["provider"]["exec"]
            .as_array_mut()
            .ok_or_else(|| anyhow!("Invalid worker configuration"))?
            .splice(1..1, [json!("--data-dir"), json!(canonical)]);
    }
    Ok(value)
}

fn write_config(path: &Path, value: &Value) -> Result<()> {
    for p in path.ancestors().filter(|p| !p.as_os_str().is_empty()) {
        match fs::symlink_metadata(p) {
            Ok(m) if m.file_type().is_symlink() => bail!("Unsafe configuration path"),
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => bail!("Cannot inspect configuration path"),
        }
    }
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file =
        options.open(path).map_err(|_| anyhow!("Cannot create configuration; output must not already exist"))?;
    let bytes = serde_json::to_vec_pretty(value).map_err(|_| anyhow!("Cannot encode configuration"))?;
    file.write_all(&bytes)
        .and_then(|_| file.write_all(b"\n"))
        .and_then(|_| file.sync_all())
        .map_err(|_| anyhow!("Cannot save configuration"))
}

async fn worker(backend: Backend) -> Result<()> {
    let mut stdin = tokio::io::BufReader::new(tokio::io::stdin());
    let mut stdout = tokio::io::stdout();
    loop {
        let mut bytes = Vec::new();
        loop {
            let available = stdin.fill_buf().await.map_err(|_| anyhow!("Cannot read worker input"))?;
            if available.is_empty() {
                if bytes.is_empty() {
                    return Ok(());
                }
                bail!("Worker input was interrupted");
            }
            let n = available.iter().position(|b| *b == b'\n').map(|n| n + 1).unwrap_or(available.len());
            if bytes.len().saturating_add(n) > 16 * 1024 * 1024 {
                bail!("Worker request is too large");
            }
            bytes.extend_from_slice(&available[..n]);
            stdin.consume(n);
            if bytes.last() == Some(&b'\n') {
                break;
            }
        }
        let request: Value = serde_json::from_slice(&bytes).map_err(|_| anyhow!("Invalid worker request"))?;
        let id = request
            .get("id")
            .filter(|v| v.is_string() || v.is_number())
            .cloned()
            .ok_or_else(|| anyhow!("Worker request needs an id"))?;
        let result = if request["event"] == "provider_complete" {
            backend.complete(request["data"].clone()).await.map(|response| json!({"version":1,"response":response}))
        } else {
            Err(anyhow!("Connection worker supports provider_complete only"))
        };
        let reply = match result {
            Ok(data) => json!({"id":id,"data":data}),
            Err(error) => json!({"id":id,"error":i_chatgpt::inference::worker_diagnostic(&error)}),
        };
        let mut line = serde_json::to_vec(&reply).map_err(|_| anyhow!("Cannot encode worker response"))?;
        line.push(b'\n');
        stdout.write_all(&line).await.map_err(|_| anyhow!("Cannot write worker response"))?;
        stdout.flush().await.map_err(|_| anyhow!("Cannot flush worker response"))?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configured_worker_pins_default_directory_without_an_explicit_override() {
        let dir = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
        let actual = resolve_data_directory(None, dir.path()).unwrap();
        let value =
            config_value_with_directory(false, "test", Path::new("/app/i-chatgpt-plugin"), Some(&actual)).unwrap();
        assert_eq!(value["provider"]["exec"][1], "--data-dir");
        assert_eq!(Path::new(value["provider"]["exec"][2].as_str().unwrap()), dir.path().canonicalize().unwrap());
        assert_eq!(value["provider"]["exec"][3], "worker");
    }

    #[test]
    fn configured_worker_resolves_relative_directory_before_changing_cwd() {
        let dir = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
        let relative = std::path::PathBuf::from(dir.path().file_name().unwrap());
        assert!(!relative.is_absolute());
        let value =
            config_value_with_directory(false, "test", Path::new("/app/i-chatgpt-plugin"), Some(&relative)).unwrap();
        let stored = Path::new(value["provider"]["exec"][2].as_str().unwrap());
        assert!(stored.is_absolute());
        assert_eq!(stored, dir.path().canonicalize().unwrap());
    }

    #[test]
    fn generated_config_uses_native_plugin_worker_without_credentials() {
        let value = config_value(false, "test", std::path::Path::new("/app/i-chatgpt-plugin"));
        assert_eq!(value["provider"]["completion"], "plugin");
        assert_eq!(value["provider"]["exec"][1], "worker");
        assert!(value.get("api_key").is_none());
        assert_eq!(value["protocol"], "responses");
    }

    #[test]
    fn configure_never_overwrites_an_existing_file() {
        let dir = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
        let path = dir.path().join("config.json");
        write_config(&path, &json!({"model":"first"})).unwrap();
        assert!(write_config(&path, &json!({"model":"second"})).is_err());
        assert_eq!(serde_json::from_slice::<Value>(&fs::read(path).unwrap()).unwrap()["model"], "first");
    }
}
