use clap::CommandFactory;
use clap_complete::{Shell, generate};
use std::fs;
use std::io;

// 复用 main.rs 中的 Args 定义
#[derive(clap::Parser)]
#[command(name = "i", version, about = "i (lninl) — plugin-first agent")]
struct Args {
    #[arg(trailing_var_arg = true)]
    task: Vec<String>,
}

fn main() -> anyhow::Result<()> {
    let mut cmd = Args::command();
    let shells = [Shell::Bash, Shell::Zsh, Shell::Fish, Shell::PowerShell];

    // 创建 completions 目录
    fs::create_dir_all("completions")?;

    for shell in shells {
        let mut buf = Vec::new();
        generate(shell, &mut cmd, "i", &mut io::Cursor::new(&mut buf));

        let ext = match shell {
            Shell::Bash => "bash",
            Shell::Zsh => "zsh",
            Shell::Fish => "fish",
            Shell::PowerShell => "ps1",
            _ => continue,
        };

        let filename = format!("completions/i.{}", ext);
        fs::write(&filename, &buf)?;
        println!("生成: {}", filename);
    }

    println!("\n安装说明：");
    println!("  Bash:  source completions/i.bash");
    println!("  Zsh:   source completions/i.zsh");
    println!("  Fish:  source completions/i.fish");
    println!("  PS:    . completions/i.ps1");

    Ok(())
}
