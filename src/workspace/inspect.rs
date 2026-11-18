use super::{Workspace, field, number};
use crate::platform::FileSystem;
use anyhow::{Context, Result, ensure};
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use regex::{Regex, RegexBuilder};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{collections::HashMap, path::PathBuf};

const SCAN_BYTES: usize = 64 * 1024 * 1024;
const ENTRIES: usize = 10_000;

// Reserve space for continuation metadata; the final JSON is never silently clipped.
struct Page {
    results: Vec<Value>,
    used: usize,
    budget: usize,
}
impl Page {
    fn new<F: FileSystem>(ws: &Workspace<F>, args: &Value) -> Result<Self> {
        let budget = number(args, "max_bytes", 12_000)?;
        ensure!((1024..=64_000).contains(&budget), "max_bytes must be 1024..64000");
        ensure!(ws.limits.output_bytes >= 1024, "tool output allowance must be at least 1024 bytes");
        Ok(Self { results: Vec::new(), used: 0, budget: budget.min(ws.limits.output_bytes) })
    }
    fn fits(&self, value: &Value) -> bool {
        self.used + value.to_string().len() < self.budget - 384
    }
    fn push(&mut self, value: Value) -> bool {
        if !self.fits(&value) {
            return false;
        }
        self.used += value.to_string().len() + 1;
        self.results.push(value);
        true
    }
    fn finish(self, next: Value, skipped: usize) -> Result<Value> {
        let output =
            json!({"results":self.results,"truncated":!next.is_null(),"next":next,"skipped":skipped}).to_string();
        ensure!(output.len() <= self.budget, "output metadata exceeds budget; narrow the request");
        Ok(json!({"output":output}))
    }
}

fn flag(args: &Value, key: &str, default: bool) -> Result<bool> {
    args.get(key).map(|v| v.as_bool().with_context(|| format!("invalid {key}"))).unwrap_or(Ok(default))
}
fn globs(patterns: &[String]) -> Result<GlobSet> {
    ensure!((1..=16).contains(&patterns.len()), "provide 1..16 glob patterns");
    let mut set = GlobSetBuilder::new();
    for pattern in patterns {
        ensure!(!pattern.is_empty() && pattern.len() <= 1024, "invalid glob length");
        set.add(GlobBuilder::new(pattern).literal_separator(true).backslash_escape(false).build()?);
    }
    Ok(set.build()?)
}
fn display(path: &std::path::Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
fn snippet(line: &str) -> String {
    super::text::clip(line, 400)
}
fn match_snippet(line: &str, start: usize) -> String {
    let begin = line.floor_char_boundary(start.saturating_sub(100));
    let end = line.floor_char_boundary((begin + 400).min(line.len()));
    format!(
        "{}{}{}",
        if begin > 0 { "[truncated] " } else { "" },
        &line[begin..end],
        if end < line.len() { " [truncated]" } else { "" }
    )
}

impl<F: FileSystem> Workspace<F> {
    pub(super) async fn list_page(&self, args: &Value) -> Result<Value> {
        let root = self.fs.resolve_path(
            args.get("path").map(|v| v.as_str().context("invalid path")).transpose()?.unwrap_or(""),
            true,
        )?;
        let mut entries = self.fs.read_dir(&root).await?;
        entries.sort();
        let cursor = number(args, "cursor", 0)?;
        let limit = number(args, "limit", 100)?;
        ensure!(cursor <= entries.len() && (1..=500).contains(&limit), "invalid cursor or limit");
        let mut page = Page::new(self, args)?;
        for (i, path) in entries.iter().enumerate().skip(cursor) {
            if page.results.len() == limit {
                return page.finish(json!(i), 0);
            }
            let meta = self.fs.metadata(path).await?.context("entry disappeared during list; retry")?;
            if !page
                .push(json!({"path":display(path),"type":if meta.is_dir {"directory"}else{"file"},"bytes":meta.len}))
            {
                ensure!(!page.results.is_empty(), "path exceeds output budget; increase max_bytes");
                return page.finish(json!(i), 0);
            }
        }
        page.finish(Value::Null, 0)
    }
    async fn scan_files(&self, args: &Value) -> Result<Vec<PathBuf>> {
        let root = self.fs.resolve_path(
            args.get("path").map(|v| v.as_str().context("invalid path")).transpose()?.unwrap_or(""),
            true,
        )?;
        let include = flag(args, "include_ignored", false)?;
        let mut dirs = vec![root];
        let mut files = Vec::new();
        let mut seen = 0;
        while let Some(dir) = dirs.pop() {
            for path in self.fs.read_dir(&dir).await? {
                seen += 1;
                ensure!(seen <= ENTRIES, "scan exceeds 10000 entries; narrow path");
                let meta = self.fs.metadata(&path).await?.context("entry disappeared during scan; retry")?;
                if meta.is_dir {
                    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    if include || ![".git", "node_modules", "target", ".i"].contains(&name) {
                        dirs.push(path);
                    }
                } else if meta.is_file {
                    files.push(path);
                }
            }
        }
        files.sort();
        Ok(files)
    }
    pub(super) async fn glob(&self, args: &Value) -> Result<Value> {
        let patterns: Vec<String> =
            serde_json::from_value(args["patterns"].clone()).context("patterns must be strings")?;
        let matcher = globs(&patterns)?;
        let files = self.scan_files(args).await?;
        let mut page = Page::new(self, args)?;
        let limit = number(args, "limit", 100)?;
        ensure!((1..=500).contains(&limit), "limit must be 1..500");
        let cursor = number(args, "cursor", 0)?;
        ensure!(cursor <= files.len(), "cursor out of range");
        for (i, path) in files.iter().enumerate().skip(cursor) {
            let path = display(path);
            if matcher.is_match(&path) && (page.results.len() == limit || !page.push(json!(path))) {
                ensure!(!page.results.is_empty(), "path exceeds output budget; increase max_bytes");
                return page.finish(json!(i), 0);
            }
        }
        page.finish(Value::Null, 0)
    }
    pub(super) async fn search(&self, args: &Value) -> Result<Value> {
        let pattern = field(args, "pattern")?;
        ensure!(!pattern.is_empty() && pattern.len() <= 1024, "pattern must be 1..1024 bytes");
        let pattern = if flag(args, "regex", false)? { pattern.to_owned() } else { regex::escape(pattern) };
        let regex: Regex = RegexBuilder::new(&pattern)
            .case_insensitive(!flag(args, "case_sensitive", true)?)
            .size_limit(1024 * 1024)
            .dfa_size_limit(1024 * 1024)
            .build()?;
        let matcher = globs(&[args
            .get("glob")
            .map(|v| v.as_str().context("invalid glob"))
            .transpose()?
            .unwrap_or("**/*")
            .to_owned()])?;
        let mode = args.get("mode").map(|v| v.as_str().context("invalid mode")).transpose()?.unwrap_or("content");
        ensure!(matches!(mode, "content" | "files"), "mode must be content or files");
        let context = number(args, "context_lines", 0)?;
        ensure!(context <= 3, "context_lines must be 0..3");
        let limit = number(args, "limit", 50)?;
        ensure!((1..=200).contains(&limit), "limit must be 1..200");
        let cursor = args.get("cursor").cloned().unwrap_or(json!({}));
        ensure!(cursor.is_object(), "invalid cursor");
        let first = number(&cursor, "file_index", 0)?;
        let first_line = number(&cursor, "line", 0)?;
        let files = self.scan_files(args).await?;
        ensure!(first <= files.len(), "cursor out of range");
        let mut page = Page::new(self, args)?;
        let mut scanned = 0;
        let mut skipped = 0;
        for (i, path) in files.iter().enumerate().skip(first) {
            let name = display(path);
            if !matcher.is_match(&name) {
                continue;
            }
            let meta = self.fs.metadata(path).await?.context("file disappeared during search; retry")?;
            if meta.len > self.limits.file_bytes as u64 {
                skipped += 1;
                continue;
            }
            if scanned + meta.len as usize > SCAN_BYTES {
                return page.finish(json!({"file_index":i,"line":0}), skipped);
            }
            let bytes = self.fs.read_file_limited(path, self.limits.file_bytes).await?;
            scanned += bytes.len();
            if bytes.contains(&0) {
                skipped += 1;
                continue;
            }
            let Ok(text) = String::from_utf8(bytes) else {
                skipped += 1;
                continue;
            };
            let lines: Vec<_> = text.lines().collect();
            for (line, text) in lines.iter().enumerate().skip(if i == first { first_line } else { 0 }) {
                let Some(found) = regex.find(text) else {
                    continue;
                };
                let mut hit = if mode == "files" {
                    json!(name)
                } else {
                    json!({"path":name,"line":line+1,"text":match_snippet(text,found.start())})
                };
                if mode == "content" && context > 0 {
                    hit["before"] =
                        json!(lines[line.saturating_sub(context)..line].iter().map(|s| snippet(s)).collect::<Vec<_>>());
                    hit["after"] = json!(
                        lines[line + 1..(line + 1 + context).min(lines.len())]
                            .iter()
                            .map(|s| snippet(s))
                            .collect::<Vec<_>>()
                    );
                }
                if page.results.len() == limit || !page.push(hit) {
                    ensure!(
                        !page.results.is_empty(),
                        "search hit exceeds output budget; reduce context_lines or increase max_bytes"
                    );
                    return page.finish(json!({"file_index":i,"line":line}), skipped);
                }
                if mode == "files" {
                    break;
                }
            }
        }
        page.finish(Value::Null, skipped)
    }
    pub(super) async fn read_batch(&self, args: &Value) -> Result<Value> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Range {
            path: String,
            start_line: Option<usize>,
            end_line: Option<usize>,
        }
        let ranges: Vec<Range> =
            serde_json::from_value(args["files"].clone()).context("files must contain file ranges")?;
        ensure!((1..=16).contains(&ranges.len()), "read accepts 1..16 ranges");
        // Validate the whole request before performing any reads.
        for range in &ranges {
            self.fs.resolve_path(&range.path, false)?;
            let start = range.start_line.unwrap_or(1);
            let end = range.end_line.unwrap_or(start.saturating_add(199));
            ensure!(start > 0 && end >= start && end - start < 1000, "invalid range; maximum 1000 lines");
        }
        let mut page = Page::new(self, args)?;
        let mut cache = HashMap::<PathBuf, String>::new();
        let mut scanned = 0;
        for (i, range) in ranges.iter().enumerate() {
            let path = self.fs.resolve_path(&range.path, false)?;
            if !cache.contains_key(&path) {
                let read: Result<String> = async {
                    let bytes = self.fs.read_file_limited(&path, self.limits.file_bytes).await?;
                    ensure!(scanned + bytes.len() <= SCAN_BYTES, "batch exceeds 64 MiB; use fewer files");
                    scanned += bytes.len();
                    String::from_utf8(bytes).context("expected UTF-8 text")
                }
                .await;
                match read {
                    Ok(text) => {
                        cache.insert(path.clone(), text);
                    }
                    Err(error) => {
                        if !page.push(json!({"path":range.path,"error":super::text::clip(&format!("{error:#}"),200)})) {
                            return page.finish(json!({"file_index":i,"start_line":range.start_line.unwrap_or(1)}), 0);
                        }
                        continue;
                    }
                }
            }
            let text = &cache[&path];
            let start = range.start_line.unwrap_or(1);
            let end = range.end_line.unwrap_or(start.saturating_add(199));
            let lines: Vec<_> = text.lines().collect();
            let mut record = json!({"path":range.path,"total_lines":lines.len(),"content":""});
            let mut content = String::new();
            for (index, line) in lines.iter().enumerate().skip(start - 1).take(end - start + 1) {
                let previous = content.len();
                content.push_str(&format!("{}: {line}\n", index + 1));
                record["content"] = json!(content);
                if !page.fits(&record) {
                    content.truncate(previous);
                    record["content"] = json!(content);
                    if previous > 0 {
                        page.push(record);
                    }
                    if page.results.is_empty() {
                        page.push(json!({"path":range.path,"error":"line exceeds output budget; increase max_bytes or use search for a snippet"}));
                    }
                    return page.finish(json!({"file_index":i,"start_line":index+1}), 0);
                }
            }
            if !page.push(record) {
                return page.finish(json!({"file_index":i,"start_line":start}), 0);
            }
        }
        page.finish(Value::Null, 0)
    }
}
