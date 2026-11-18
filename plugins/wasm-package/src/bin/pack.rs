fn main() {
    if let Err(error) = run() {
        eprintln!("{error:#}");
        std::process::exit(1);
    }
}
fn run() -> anyhow::Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    anyhow::ensure!(args.len() == 2, "Usage: i-plugin-pack INPUT_DIR OUTPUT.hplugin.zip");
    i_wasm_package::pack(std::path::Path::new(&args[0]), std::path::Path::new(&args[1]))
}
