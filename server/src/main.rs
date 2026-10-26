use i_relay::{Config, Relay, serve};
use std::{env, sync::Arc};
use tokio::net::TcpListener;

async fn run() -> Result<(), &'static str> {
    if env::var_os("HARNESS_STATIC_ROOT").is_some() {
        return Err("Static files are served by Caddy; remove HARNESS_STATIC_ROOT");
    }
    let token = env::var("HARNESS_RELAY_TOKEN").map_err(|_| "HARNESS_RELAY_TOKEN is required")?;
    let origin = env::var("HARNESS_PUBLIC_ORIGIN").map_err(|_| "HARNESS_PUBLIC_ORIGIN is required")?;
    let endpoints: Vec<String> =
        serde_json::from_str(&env::var("HARNESS_UPSTREAMS").map_err(|_| "HARNESS_UPSTREAMS is required")?)
            .map_err(|_| "HARNESS_UPSTREAMS must be a JSON array of endpoint strings")?;
    let prefix = env::var("HARNESS_BASE_PATH").unwrap_or_else(|_| "/".to_owned());
    let config = Config::new(&token, &origin, &endpoints, &prefix)?;
    let listener = TcpListener::bind("127.0.0.1:8080").await.map_err(|_| "Cannot listen on 127.0.0.1:8080")?;
    eprintln!("Relay listening on 127.0.0.1:8080; serve static files with your HTTPS reverse proxy.");
    serve(listener, Arc::new(Relay::new(config)), async {
        let _ = tokio::signal::ctrl_c().await;
    })
    .await
    .map_err(|_| "Relay listener failed")
}

#[tokio::main]
async fn main() {
    let args: Vec<_> = env::args_os().skip(1).collect();
    if args.len() == 1 && (args[0] == "--help" || args[0] == "-h") {
        println!(
            "i-relay: private same-origin model API relay\n\nRun: i-relay\nRequired environment: HARNESS_RELAY_TOKEN (32+ bytes), HARNESS_PUBLIC_ORIGIN, HARNESS_UPSTREAMS (JSON array).\nOptional: HARNESS_BASE_PATH (default /). Listens only on 127.0.0.1:8080.\nServe static files and HTTPS with Caddy. No credentials in command arguments.\nSee https://github.com/qinains/i/tree/main/server"
        );
        return;
    }
    if args.len() == 1 && args[0] == "--version" {
        println!("i-relay {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    if !args.is_empty() {
        eprintln!(
            "Unknown arguments. Use i-relay --help. Pass credentials through your secret manager, not command arguments."
        );
        std::process::exit(1);
    }
    if let Err(reason) = run().await {
        eprintln!("Relay failed: {reason}. See server/README.md. Configuration values are not logged.");
        std::process::exit(1);
    }
}
