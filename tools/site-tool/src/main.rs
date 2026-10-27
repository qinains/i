use anyhow::{Context, Result, bail};
use std::path::PathBuf;
fn main() -> Result<()> {
    let mut args = std::env::args().skip(1).collect::<Vec<_>>();
    const USAGE: &str = "i-site-tool [--root PATH] translations [--check] | site [--origin URL] [--output PATH] | cors-extension | cors-safari [--build] | serve [--directory PATH] [--port PORT]";
    if args.iter().any(|argument| matches!(argument.as_str(), "--help" | "-h")) {
        println!("{USAGE}");
        return Ok(());
    }
    let mut root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    if let Some(index) = args.iter().position(|s| s == "--root") {
        root = PathBuf::from(args.get(index + 1).context("--root requires a path")?);
        args.drain(index..index + 2);
    }
    let command = args.first().context(USAGE)?.clone();
    args.remove(0);
    match command.as_str() {
        "translations" => {
            let check = args == ["--check"];
            if !args.is_empty() && !check {
                bail!("unexpected translations arguments");
            }
            i_site_tool::translations::generate(&root, check)?;
            println!("Translation catalogs {}", if check { "verified" } else { "generated" });
        }
        "cors-extension" => {
            if !args.is_empty() {
                bail!("unexpected cors-extension arguments");
            }
            println!("{}", serde_json::to_string_pretty(&i_site_tool::cors::build(&root)?)?);
        }
        "cors-safari" => {
            let build = args == ["--build"];
            if !args.is_empty() && !build {
                bail!("unexpected cors-safari arguments");
            }
            println!("{}", i_site_tool::cors::safari(&root, build)?);
        }
        "site" => {
            let mut origin = "https://i.lninl.com".to_owned();
            let mut output = root.join("dist/site");
            let mut index = 0;
            while index < args.len() {
                let value = args.get(index + 1).context("option requires value")?;
                match args[index].as_str() {
                    "--origin" => origin = value.clone(),
                    "--output" => output = value.into(),
                    _ => bail!("unexpected site option"),
                };
                index += 2;
            }
            let count = i_site_tool::site::build(&root, &origin, &output)?;
            println!("Built {count} documentation pages and verified /app/ assets into {}", output.display());
        }
        "serve" => {
            let mut directory = root.join("dist/site");
            let mut port = 8090;
            let mut index = 0;
            while index < args.len() {
                let value = args.get(index + 1).context("option requires value")?;
                match args[index].as_str() {
                    "--directory" => directory = root.join(value),
                    "--port" => port = value.parse().context("invalid port")?,
                    _ => bail!("unexpected serve option"),
                };
                index += 2;
            }
            i_site_tool::serve::run(&directory, port)?;
        }
        _ => bail!("unknown command: {command}"),
    }
    Ok(())
}
