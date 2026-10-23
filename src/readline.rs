use anyhow::Result;
use rustyline::completion::{Completer, Pair};
use rustyline::error::ReadlineError;
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::validate::Validator;
use rustyline::{Config, Context, Editor, Helper};
use std::borrow::Cow;
use std::process::Command;

fn slash_commands() -> Vec<(String, String)> {
    let entries: serde_json::Value =
        serde_json::from_str(include_str!("../web/commands.json")).expect("valid shared slash command catalog");
    entries
        .as_array()
        .expect("command catalog array")
        .iter()
        .filter(|entry| {
            entry["platforms"].as_array().is_some_and(|values| values.iter().any(|value| value == "readline"))
        })
        .map(|entry| (entry["name"].as_str().unwrap().into(), entry["description"].as_str().unwrap().into()))
        .collect()
}

struct CmdCompleter;

impl Completer for CmdCompleter {
    type Candidate = Pair;

    fn complete(&self, line: &str, pos: usize, _ctx: &Context<'_>) -> rustyline::Result<(usize, Vec<Pair>)> {
        if line.starts_with('/') && pos == line.len() && !line.contains(char::is_whitespace) {
            let matches: Vec<Pair> = slash_commands()
                .iter()
                .filter(|(cmd, _)| cmd.starts_with(line))
                .map(|(cmd, desc)| Pair { display: format!("{:<12} {}", cmd, desc), replacement: cmd.to_string() })
                .collect();
            Ok((0, matches))
        } else {
            Ok((pos, vec![]))
        }
    }
}

struct ReadlineHelper;

impl Helper for ReadlineHelper {}

impl Completer for ReadlineHelper {
    type Candidate = Pair;

    fn complete(&self, line: &str, pos: usize, _ctx: &Context<'_>) -> rustyline::Result<(usize, Vec<Pair>)> {
        CmdCompleter.complete(line, pos, _ctx)
    }
}

impl Hinter for ReadlineHelper {
    type Hint = String;

    fn hint(&self, _line: &str, _pos: usize, _ctx: &Context<'_>) -> Option<String> {
        None
    }
}

impl Highlighter for ReadlineHelper {
    fn highlight_prompt<'b, 's: 'b, 'p: 'b>(&'s self, prompt: &'p str, _default: bool) -> Cow<'b, str> {
        Cow::Borrowed(prompt)
    }
}

impl Validator for ReadlineHelper {}

pub fn read_input(prompt: &str) -> Result<String> {
    let config = Config::builder().auto_add_history(false).build();

    let mut editor: Editor<ReadlineHelper, rustyline::history::FileHistory> = Editor::with_config(config)?;
    editor.set_helper(Some(ReadlineHelper));

    // 加载历史记录
    let history_path = dirs_next::home_dir().map(|h| h.join(".i").join("history")).unwrap_or_default();
    let _ = editor.load_history(&history_path);

    loop {
        match editor.readline(prompt) {
            Ok(line) => {
                let trimmed = line.trim();

                // 处理 ! 前缀的系统命令
                if let Some(command) = trimmed.strip_prefix('!') {
                    execute_shell_command(command)?;
                    continue;
                }

                // 处理 / 斜杠命令
                if trimmed.starts_with('/') && handle_slash_command(trimmed)? {
                    continue;
                }

                // 保存到历史记录
                let _ = editor.add_history_entry(trimmed);
                let _ = editor.save_history(&history_path);

                return Ok(trimmed.to_string());
            }
            Err(ReadlineError::Interrupted) => {
                // Ctrl+C
                eprintln!("^C");
                continue;
            }
            Err(ReadlineError::Eof) => {
                // Ctrl+D
                return Ok(String::new());
            }
            Err(err) => {
                return Err(err.into());
            }
        }
    }
}

fn execute_shell_command(cmd: &str) -> Result<()> {
    let shell = if cfg!(windows) { "cmd" } else { "sh" };
    let flag = if cfg!(windows) { "/C" } else { "-c" };

    let status = Command::new(shell).arg(flag).arg(cmd).status()?;

    if !status.success() {
        eprintln!("Command exit code: {}", status.code().unwrap_or(-1));
    }
    Ok(())
}

fn handle_slash_command(cmd: &str) -> Result<bool> {
    match cmd {
        "/exit" => {
            std::process::exit(0);
        }
        "/clear" => {
            print!("\x1b[2J\x1b[H");
            Ok(true)
        }
        "/help" => {
            eprintln!("\nAvailable slash commands:");
            for (cmd, desc) in slash_commands() {
                eprintln!("  {:<12} {}", cmd, desc);
            }
            eprintln!();
            Ok(true)
        }
        "/resume" => {
            eprintln!("\nAvailable sessions:");
            let session_dir = dirs_next::home_dir().map(|h| h.join(".i").join("sessions")).unwrap_or_default();
            if session_dir.exists() {
                let entries: Vec<_> = std::fs::read_dir(&session_dir)?
                    .filter_map(|e| e.ok())
                    .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
                    .collect();
                if entries.is_empty() {
                    eprintln!("  No saved sessions found.");
                } else {
                    let mut sessions: Vec<(usize, String)> = entries
                        .iter()
                        .enumerate()
                        .map(|(i, e)| {
                            let name = e.file_name().to_string_lossy().trim_end_matches(".json").to_string();
                            (i + 1, name)
                        })
                        .collect();
                    sessions.sort_by(|a, b| a.1.cmp(&b.1));
                    for (num, name) in &sessions {
                        eprintln!("  [{}] {}", num, name);
                    }
                    eprintln!("\n/resume only lists saved sessions in this client.");
                }
            } else {
                eprintln!("  No saved sessions found.");
            }
            eprintln!();
            Ok(true)
        }
        _ => {
            eprintln!("Unknown command: {}", cmd);
            eprintln!("Type /help to see available commands");
            Ok(true)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_catalog_only_advertises_implemented_commands() {
        let names: Vec<_> = slash_commands().into_iter().map(|entry| entry.0).collect();
        assert_eq!(names, ["/help", "/clear", "/resume", "/exit"]);
        assert!(handle_slash_command("/resume 1").unwrap());
    }

    #[test]
    fn fallback_completion_is_only_for_a_leading_command_prefix() {
        let history = rustyline::history::DefaultHistory::new();
        let context = Context::new(&history);
        let (_, candidates) = CmdCompleter.complete("/re", 3, &context).unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].replacement, "/resume");
        for line in ["path /re", "/resume 1", " /re", "/setup"] {
            assert!(CmdCompleter.complete(line, line.len(), &context).unwrap().1.is_empty());
        }
        assert!(CmdCompleter.complete("/resume", 2, &context).unwrap().1.is_empty());
    }
}
