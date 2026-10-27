#[path = "../backend.rs"]
mod backend;
#[path = "../bridge.rs"]
mod bridge;
#[path = "../claude.rs"]
mod claude;
#[path = "../cli.rs"]
mod cli;

#[tokio::main]
async fn main() {
    if let Err(error) = cli::run(true).await {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
