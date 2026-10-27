use anyhow::{Result, ensure};
use std::{fs, path::Path};
use walkdir::WalkDir;

const SKIP: &[&str] = &[
    ".git",
    "target",
    "dist",
    "node_modules",
    ".gstack",
    ".superpowers",
    "superpowers",
    ".i",
    ".1k",
    "__pycache__",
    "test-results",
    "playwright-report",
    "xcuserdata",
];

fn interpreter(word: &str) -> bool {
    let word = word.trim_matches(['\'', '"', '`', '@']).replace('\\', "/").to_ascii_lowercase();
    let name = word.rsplit('/').next().unwrap_or("").trim_end_matches(".exe");
    name == "py"
        || name == "python"
        || name == "pythonw"
        || name.strip_prefix("python").is_some_and(|suffix| {
            !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit() || byte == b'.')
        })
}

fn shell_words(line: &str) -> Vec<String> {
    let (mut words, mut word, mut quote) = (Vec::new(), String::new(), None);
    for character in line.chars() {
        if let Some(end) = quote {
            word.push(character);
            if character == end {
                quote = None;
            }
        } else if matches!(character, '\'' | '"') {
            quote = Some(character);
            word.push(character);
        } else if character.is_whitespace() || matches!(character, ';' | '&' | '|') {
            if !word.is_empty() {
                words.push(std::mem::take(&mut word));
            }
            if !character.is_whitespace() {
                words.push(character.to_string());
            }
        } else if character == '#' && word.is_empty() {
            break;
        } else {
            word.push(character);
        }
    }
    if !word.is_empty() {
        words.push(word);
    }
    words
}

fn shell_invocation(line: &str) -> bool {
    let mut line = line.trim();
    if let Some(line) = line.strip_prefix("#!") {
        return line.split_whitespace().any(interpreter);
    }
    if line.starts_with('#') {
        return false;
    }
    if let Some((prefix, command)) = line.split_once("run:")
        && prefix.trim().trim_start_matches('-').trim().is_empty()
    {
        line = command.trim();
    }
    let words = shell_words(line);
    let mut command = true;
    for word in words {
        if command && interpreter(&word) {
            return true;
        }
        if command && word.contains('=') {
            continue;
        }
        command =
            matches!(word.rsplit('/').next().unwrap_or(""), ";" | "|" | "exec" | "env" | "then" | "do" | "sudo" | "&");
    }
    false
}

fn process_invocation(source: &str) -> bool {
    for function in
        ["Command::new", "Command :: new", "spawn", "spawnSync", "exec", "execSync", "execFile", "execFileSync"]
    {
        for (index, _) in source.match_indices(function) {
            if index > 0 && source.as_bytes()[index - 1].is_ascii_alphanumeric() {
                continue;
            }
            let suffix = source[index + function.len()..].trim_start();
            let Some(suffix) = suffix.strip_prefix('(') else {
                continue;
            };
            let mut argument = suffix.trim_start();
            // Rust raw string command names are just as executable as ordinary literals.
            if let Some(raw) = argument.strip_prefix('r') {
                argument = raw.trim_start_matches('#');
            }
            let Some(quote) = argument.chars().next().filter(|c| ['\'', '"', '`'].contains(c)) else {
                continue;
            };
            if let Some(end) = argument[1..].find(quote) {
                let command = &argument[1..1 + end];
                if shell_invocation(command) {
                    return true;
                }
            }
        }
    }
    false
}

fn legacy_script(source: &str) -> bool {
    let normalized = source.replace('\\', "/");
    normalized.match_indices("scripts/").any(|(index, _)| {
        let path = normalized[index..]
            .split(|ch: char| !ch.is_ascii_alphanumeric() && !"_./*?-".contains(ch))
            .next()
            .unwrap_or("");
        path.ends_with(".py") || path.ends_with(".pyc")
    })
}

fn check(root: &Path) -> Result<()> {
    let mut errors = Vec::new();
    for entry in WalkDir::new(root).follow_links(false).into_iter().filter_entry(|entry| {
        entry.depth() == 0
            || (!SKIP.contains(&entry.file_name().to_string_lossy().as_ref())
                && entry.path().strip_prefix(root).is_ok_and(|relative| {
                    !["web/pkg", "web/plugin-pkg", "web/downloads"].iter().any(|path| relative.starts_with(path))
                }))
    }) {
        let entry = entry?;
        if entry.file_type().is_dir() {
            continue;
        }
        let relative = entry.path().strip_prefix(root)?;
        let extension = entry.path().extension().and_then(|s| s.to_str()).unwrap_or("").to_ascii_lowercase();
        if matches!(extension.as_str(), "py" | "pyc") {
            errors.push(format!("{}: repository Python source/bytecode", relative.display()));
            continue;
        }
        if entry.file_type().is_symlink() {
            continue;
        }
        let text_file = matches!(
            extension.as_str(),
            "rs" | "mjs"
                | "cjs"
                | "js"
                | "sh"
                | "bash"
                | "zsh"
                | "ps1"
                | "cmd"
                | "bat"
                | "yml"
                | "yaml"
                | "toml"
                | "json"
                | "md"
                | "html"
        );
        if !text_file {
            continue;
        }
        let bytes = fs::read(entry.path())?;
        let Ok(source) = std::str::from_utf8(&bytes) else {
            continue;
        };
        if legacy_script(source) {
            errors.push(format!("{}: legacy repository script reference", relative.display()));
        }
        let shell = matches!(extension.as_str(), "sh" | "bash" | "zsh" | "ps1" | "cmd" | "bat" | "yml" | "yaml");
        let process = matches!(extension.as_str(), "rs" | "mjs" | "cjs" | "js");
        if shell && source.lines().any(shell_invocation) || process && process_invocation(source) {
            errors.push(format!("{}: Python interpreter entrypoint", relative.display()));
        }
    }
    ensure!(errors.is_empty(), "Repository Python dependency policy failed:\n{}", errors.join("\n"));
    Ok(())
}

pub(crate) fn run(root: &Path) -> Result<()> {
    check(root)?;
    println!("PASS: no repository Python source, legacy script references or interpreter entrypoints");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn permits_language_descriptions_and_skips_generated_dependency_trees() {
        let root = tempfile::tempdir().unwrap();
        fs::write(
            root.path().join("README.md"),
            "Users may write Python plugins; this repository does not need Python.",
        )
        .unwrap();
        fs::write(root.path().join("test.rs"), "let language = \"Python\"; // user language support").unwrap();
        for dir in ["target", "dist", "node_modules", ".git", ".gstack", "__pycache__"] {
            fs::create_dir(root.path().join(dir)).unwrap();
            fs::write(root.path().join(dir).join("generated.py"), "external/generated fixture").unwrap();
        }
        assert!(check(root.path()).is_ok());
    }

    #[test]
    fn rejects_python_source_bytecode_and_legacy_script_references() {
        for name in ["fixture.py", "fixture.pyc"] {
            let root = tempfile::tempdir().unwrap();
            fs::write(root.path().join(name), "fixture").unwrap();
            assert!(check(root.path()).is_err(), "{name}");
        }
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("README.md"), ["Run scripts/retired", ".py --check"].concat()).unwrap();
        assert!(check(root.path()).is_err());
    }

    #[test]
    fn rejects_actual_shell_workflow_node_and_rust_interpreter_entrypoints() {
        let interpreter = ["py", "thon3"].concat();
        for (name, contents) in [
            ("fixture.sh", format!("#!/bin/sh\n{interpreter} -m unittest\n")),
            ("fixture.yml", format!("steps:\n  - run: {interpreter} fixture\n")),
            ("fixture.mjs", format!("spawn('{interpreter}', ['fixture']);")),
            ("fixture.rs", format!("Command::new(\"{interpreter}\").arg(\"fixture\");")),
        ] {
            let root = tempfile::tempdir().unwrap();
            fs::write(root.path().join(name), contents).unwrap();
            assert!(check(root.path()).is_err(), "{name}");
        }
    }

    #[test]
    fn shell_control_operators_do_not_confuse_quoted_language_descriptions_with_commands() {
        let interpreter = ["py", "thon3"].concat();
        assert!(shell_invocation(&format!("set -e;{interpreter} fixture")));
        assert!(shell_invocation(&format!("echo ok&&{interpreter} fixture")));
        assert!(shell_invocation(&format!("/usr/bin/env {interpreter} fixture")));
        assert!(!shell_invocation(&format!("echo 'User can run {interpreter}; language support is allowed'")));
        assert!(!shell_invocation("echo 'Python support remains available'"));
    }
}
