use anyhow::{Result, bail};
use std::path::PathBuf;

mod bench;
mod export;
mod globset;
mod licenses;
mod python_policy;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().expect("repository root")
}

fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("licenses") => licenses::run(&root(), &args[1..]),
        Some("export-release") => export::run(&root(), &args[1..]),
        Some("check-globset") if args.len() == 1 => globset::run(&root()),
        Some("check-no-python") if args.len() == 1 => python_policy::run(&root()),
        Some("bench-search") => bench::run(&args[1..]),
        _ => bail!(
            "usage: i-repo-tool licenses [--check] [--fetch-missing] [--manifest-path FILE --output FILE] [--include-license FILE] | export-release | check-globset | check-no-python | bench-search BINARY [BINARY ...] [--runs 1..100]"
        ),
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error:#}");
        std::process::exit(1);
    }
}
