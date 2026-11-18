use super::{Workspace, path};
use crate::platform::FileSystem;
use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Cursor, Write},
    path::PathBuf,
};
use zip::{CompressionMethod, DateTime, ZipWriter, write::SimpleFileOptions};

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DownloadRequest {
    pub paths: Option<Vec<String>>,
    pub filename: Option<String>,
}
pub struct Archive {
    pub filename: String,
    pub bytes: Vec<u8>,
    pub entries: usize,
}
pub struct ArchiveLimits {
    pub bytes: usize,
    pub entries: usize,
}
impl Default for ArchiveLimits {
    fn default() -> Self {
        Self { bytes: 64 * 1024 * 1024, entries: 10000 }
    }
}
impl<F: FileSystem> Workspace<F> {
    pub async fn archive(&self, request: &DownloadRequest) -> Result<Archive> {
        self.archive_with_limits(request, ArchiveLimits::default()).await
    }
    pub async fn archive_with_limits(&self, request: &DownloadRequest, limits: ArchiveLimits) -> Result<Archive> {
        let filename = request.filename.clone().unwrap_or_else(|| "workspace.zip".into());
        ensure!(
            !filename.contains('/') && filename.ends_with(".zip") && filename.len() > 4,
            "filename must be a ZIP basename"
        );
        path::validate(&filename, false)?;
        let mut stack: Vec<PathBuf> = match &request.paths {
            Some(paths) => {
                ensure!(!paths.is_empty(), "paths must not be empty");
                paths.iter().map(|p| self.fs.resolve_path(p, false)).collect::<Result<_>>()?
            }
            None => vec![PathBuf::new()],
        };
        let mut visited = BTreeSet::new();
        let mut entries = BTreeMap::new();
        while let Some(p) = stack.pop() {
            if request.paths.is_none() && p.starts_with("/tmp") {
                continue;
            }
            if self.exclude_downloads
                && request.paths.is_none()
                && (p.starts_with(".i/downloads") || p.starts_with(".i/attachments"))
            {
                continue;
            }
            if !visited.insert(p.clone()) {
                continue;
            }
            let m = self.fs.metadata(&p).await?.context("archive path not found")?;
            if !p.as_os_str().is_empty() {
                ensure!(entries.len() < limits.entries, "archive exceeds entry limit");
                let raw = p.to_str().context("non-Unicode archive path")?;
                #[cfg(windows)]
                let normalized = raw.replace('\\', "/");
                #[cfg(windows)]
                let raw = normalized.as_str();
                // The browser's scratch mount is absolute internally, but ZIP entries must be relative.
                let raw = if p.starts_with("/tmp") { raw.trim_start_matches('/') } else { raw };
                let name = path::validate(raw, false)?.to_str().context("non-Unicode archive path")?.replace('\\', "/");
                ensure!(!entries.contains_key(&name), "archive paths collide: {name}");
                entries.insert(name, (p.clone(), m.is_dir));
            }
            if m.is_dir {
                stack.extend(self.fs.read_dir(&p).await?);
            } else {
                ensure!(m.is_file, "archive expects regular files");
            }
        }
        let options = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Stored)
            .last_modified_time(DateTime::default());
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        let mut total = 0;
        for (name, (p, is_dir)) in &entries {
            if *is_dir {
                writer.add_directory(format!("{name}/"), options)?;
            } else {
                let remaining = limits.bytes.saturating_sub(total);
                let m = self.fs.metadata(p).await?.context("archive file disappeared")?;
                ensure!(m.is_file && m.len <= remaining as u64, "archive exceeds byte limit or file changed");
                let bytes = self.fs.read_file_limited(p, remaining).await?;
                total += bytes.len();
                ensure!(total <= limits.bytes, "archive exceeds byte limit");
                writer.start_file(name.as_str(), options)?;
                writer.write_all(&bytes)?;
            }
        }
        Ok(Archive { filename, bytes: writer.finish()?.into_inner(), entries: entries.len() })
    }
}
