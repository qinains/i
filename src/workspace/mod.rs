pub mod archive;
mod inspect;
pub mod path;
pub mod text;

use crate::platform::{Capabilities, FileSystem};
use anyhow::{Context, Result, bail, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};

#[derive(Clone, Copy)]
pub struct Limits {
    pub file_bytes: usize,
    pub write_bytes: usize,
    pub output_bytes: usize,
}
impl Limits {
    pub fn browser() -> Self {
        Self { file_bytes: 8 * 1024 * 1024, write_bytes: 8 * 1024 * 1024, output_bytes: 1024 * 1024 }
    }
}
pub struct Workspace<F> {
    pub fs: F,
    pub limits: Limits,
    pub exclude_downloads: bool,
}
impl<F: FileSystem> Workspace<F> {
    pub fn new(fs: F, limits: Limits) -> Self {
        Self { fs, limits, exclude_downloads: false }
    }
    pub async fn execute(&self, name: &str, args: &Value) -> Result<Value> {
        ensure!(args.is_object(), "tool arguments must be an object");
        match name {
            "glob" => return self.glob(args).await,
            "search" => return self.search(args).await,
            "read" => return self.read_batch(args).await,
            "list" => return self.list_page(args).await,
            _ => {}
        }
        let output = match name {
            "write" | "edit" | "view_image" => {
                let path = self.fs.resolve_path(field(args, "path")?, false)?;
                if name == "write" {
                    let content = field(args, "content")?;
                    ensure!(content.len() <= self.limits.write_bytes, "file exceeds byte limit");
                    self.fs.write_file(&path, content.as_bytes()).await?;
                    format!("Wrote {} bytes to {}", content.len(), path.display())
                } else {
                    let bytes = self.fs.read_file_limited(&path, self.limits.file_bytes).await?;
                    if name == "view_image" {
                        ensure!(bytes.len() <= 8 * 1024 * 1024, "image exceeds 8 MiB");
                        let mime = text::image_mime(&bytes)?;
                        return Ok(json!({"output":"Image attached.","images":[{"type":"image_url","image_url":{
                            "url":format!("data:{mime};base64,{}", STANDARD.encode(bytes))}}]}));
                    }
                    let original = String::from_utf8(bytes).context("expected UTF-8 text")?;
                    if name == "edit" {
                        #[derive(serde::Deserialize)]
                        #[serde(deny_unknown_fields)]
                        struct Edit {
                            old: String,
                            new: String,
                            line_number: Option<usize>,
                        }
                        let edits: Vec<Edit> =
                            serde_json::from_value(args["edits"].clone()).context("edits must be an array")?;
                        ensure!((1..=64).contains(&edits.len()), "provide 1..64 edits");
                        let mut updated = original.clone();
                        for edit in &edits {
                            updated = text::replace_once(&updated, &edit.old, &edit.new, edit.line_number)?;
                            ensure!(updated.len() <= self.limits.write_bytes, "file exceeds byte limit");
                        }
                        ensure!(updated.len() <= self.limits.write_bytes, "file exceeds byte limit");
                        ensure!(
                            self.fs.read_file_limited(&path, self.limits.file_bytes).await? == original.as_bytes(),
                            "file changed during edit; retry"
                        );
                        self.fs.write_file(&path, updated.as_bytes()).await?;
                        format!("Edited {} ({} replacements)", path.display(), edits.len())
                    } else {
                        unreachable!()
                    }
                }
            }
            _ => bail!("unsupported tool: {name}"),
        };
        Ok(json!({"output":text::clip(&output, self.limits.output_bytes)}))
    }
}
pub fn field<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value[key].as_str().with_context(|| format!("missing string argument: {key}"))
}
pub fn number(value: &Value, key: &str, default: usize) -> Result<usize> {
    match value.get(key) {
        None => Ok(default),
        Some(v) => usize::try_from(v.as_u64().with_context(|| format!("invalid {key}"))?).context("number too large"),
    }
}
pub fn tool_schemas(cap: Capabilities) -> Value {
    let mut tools: Vec<Value> = serde_json::from_str(include_str!("../tools.json")).expect("valid schemas");
    tools.retain(|t| match t["function"]["name"].as_str().unwrap_or("") {
        "shell" => cap.processes,
        "download" => cap.download && cap.files,
        _ => cap.files,
    });
    json!(tools)
}
