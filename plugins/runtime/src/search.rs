use super::{state, text};
use anyhow::{Result, ensure};
use globset::Glob;
use regex::RegexBuilder;
use serde_json::{Value, json};
use std::path::Path;
use walkdir::WalkDir;

pub(crate) fn handle(event: &str, data: Value, workspace: &Path) -> Result<Value> {
    if event != "tool" {
        return Ok(data);
    }
    let name = text(&data, "name")?;
    ensure!(["search", "find_files"].contains(&name), "unknown search tool");
    let args = &data["arguments"];
    for key in ["path", "glob"] {
        ensure!(args.get(key).is_none_or(Value::is_string), "{key} must be a string");
    }
    for key in ["regex", "case_sensitive"] {
        ensure!(args.get(key).is_none_or(Value::is_boolean), "{key} must be a boolean");
    }
    let root = dunce::canonicalize(workspace)?;
    let folder = state::inside(&root, Path::new(args["path"].as_str().unwrap_or(".")))?;
    ensure!(folder.is_dir(), "search path must be a directory");
    let limit = args
        .get("limit")
        .map_or(Some(50), Value::as_u64)
        .filter(|n| (1..=200).contains(n))
        .ok_or_else(|| anyhow::anyhow!("limit must be 1..200"))? as usize;
    let glob = Glob::new(args["glob"].as_str().unwrap_or("*"))?.compile_matcher();
    let query = if name == "search" { text(args, "query")? } else { "" };
    ensure!(
        name != "search" || (!query.is_empty() && query.chars().count() <= 512),
        "query must contain 1..512 characters"
    );
    let expression = if args["regex"] == true { query.to_string() } else { regex::escape(query) };
    let regex = RegexBuilder::new(&expression)
        .case_insensitive(args["case_sensitive"] != true)
        .size_limit(4 * 1024 * 1024)
        .build()?;
    let (mut matches, mut scanned, mut skipped, mut truncated, mut visited) = (Vec::new(), 0, 0, false, 0);
    let walk = WalkDir::new(&folder).follow_links(false).sort_by_file_name().into_iter().filter_entry(|entry| {
        entry.depth() == 0
            || (!entry.file_type().is_symlink()
                && !entry.file_name().to_string_lossy().starts_with('.')
                && !["target", "node_modules", "__pycache__"].contains(&entry.file_name().to_string_lossy().as_ref())
                && dunce::canonicalize(entry.path()).is_ok_and(|path| path.starts_with(&root)))
    });
    for entry in walk {
        visited += 1;
        if visited > 20000 || scanned >= 10000 {
            truncated = true;
            break;
        }
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => {
                skipped += 1;
                continue;
            }
        };
        if !entry.file_type().is_file() {
            continue;
        }
        let display = entry
            .path()
            .strip_prefix(&root)?
            .components()
            .map(|component| component.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        if !glob.is_match(&display) {
            continue;
        }
        if name == "find_files" {
            scanned += 1;
            matches.push(json!({"path":display}));
        } else {
            let bytes = match state::read_bounded(entry.path(), 1_000_000) {
                Ok(bytes) if !bytes.contains(&0) => bytes,
                _ => {
                    skipped += 1;
                    continue;
                }
            };
            scanned += 1;
            for (line_number, line) in String::from_utf8_lossy(&bytes).lines().enumerate() {
                if let Some(found) = regex.find(line) {
                    let before = line[..found.start()].chars().count();
                    let snippet: String = line.chars().skip(before.saturating_sub(100)).take(400).collect();
                    matches.push(json!({"path":display,"line":line_number+1,"text":snippet}));
                    if matches.len() >= limit {
                        break;
                    }
                }
            }
        }
        if matches.len() >= limit {
            truncated = true;
            break;
        }
    }
    Ok(
        json!({"output":json!({"matches":matches,"files_scanned":scanned,"skipped":skipped,"truncated":truncated}).to_string()}),
    )
}
