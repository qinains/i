mod backend;
mod bridge;
mod claude;
mod cli;

#[tokio::main]
async fn main() {
    if let Err(error) = cli::run(false).await {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
