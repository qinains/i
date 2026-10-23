//! Platform-owned locations. Mobile hosts can construct these paths without HOME.
use super::{context::digest, state};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    env, fs,
    path::{Component, Path, PathBuf},
};

fn normalize(path: &Path) -> Result<PathBuf> {
    ensure!(!path.components().any(|part| part == Component::ParentDir), "platform paths must not contain '..'");
    let mut ancestor = path;
    loop {
        match fs::symlink_metadata(ancestor) {
            Ok(_) => break,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                ancestor = ancestor.parent().context("missing ancestor")?
            }
            Err(error) => return Err(error.into()),
        }
    }
    Ok(dunce::canonicalize(ancestor)?.join(path.strip_prefix(ancestor)?))
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct AppPaths {
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub runtime_dir: PathBuf,
}
impl AppPaths {
    pub(crate) fn desktop() -> Result<Self> {
        let base =
            env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(|home| PathBuf::from(home).join(".i"));
        let path = |key, name| {
            env::var_os(key)
                .map(PathBuf::from)
                .or_else(|| base.as_ref().map(|base| base.join(name)))
                .with_context(|| format!("provide {key} or HOME/USERPROFILE"))
        };
        let result = Self {
            config_dir: path("I_CONFIG_DIR", "config")?,
            data_dir: path("I_DATA_DIR", "data")?,
            cache_dir: path("I_CACHE_DIR", "cache")?,
            runtime_dir: path("I_RUNTIME_DIR", "runtime")?,
        };
        result.validate()?;
        Ok(result)
    }
    pub(crate) fn validate(&self) -> Result<()> {
        ensure!(
            [&self.config_dir, &self.data_dir, &self.cache_dir, &self.runtime_dir].iter().all(|p| p.is_absolute()),
            "application directories must be absolute platform-provided paths"
        );
        let normalized: Vec<_> = [&self.config_dir, &self.data_dir, &self.cache_dir, &self.runtime_dir]
            .into_iter()
            .map(|path| normalize(path))
            .collect::<Result<_>>()?;
        for persistent in &normalized[..2] {
            ensure!(
                !persistent.starts_with(&normalized[2]) && !persistent.starts_with(&normalized[3]),
                "persistent directories cannot live inside disposable cache/runtime directories"
            );
        }
        Ok(())
    }
    pub(crate) fn workspace(&self, root: &Path, id: Option<&str>) -> Result<(String, PathBuf)> {
        self.validate()?;
        fs::create_dir_all(&self.data_dir)?;
        let id = if let Some(id) = id {
            uuid::Uuid::parse_str(id)?.to_string()
        } else {
            let key = digest(&json!(dunce::canonicalize(root)?));
            let registry = state::inside(&self.data_dir, Path::new("workspaces.json"))?;
            let _guard = state::Lock::acquire(&registry.with_extension("lock"))?;
            let mut index: Value = if registry.try_exists()? {
                serde_json::from_slice(&state::read_bounded(&registry, 2_000_000)?)?
            } else {
                json!({})
            };
            let object = index.as_object_mut().context("invalid workspace registry")?;
            let changed = !object.contains_key(&key);
            let id = object.entry(key).or_insert_with(|| json!(uuid::Uuid::new_v4().to_string()));
            let id = uuid::Uuid::parse_str(id.as_str().context("invalid workspace id")?)?.to_string();
            ensure!(object.len() <= 10000, "workspace registry full");
            if changed {
                state::atomic(&registry, &index)?;
            }
            id
        };
        let directory = state::inside(&self.data_dir, Path::new(&format!("workspaces/{id}")))?;
        fs::create_dir_all(&directory)?;
        Ok((id, directory))
    }
    pub(crate) fn describe(&self, root: &Path) -> Result<Value> {
        let (id, directory) = self.workspace(root, None)?;
        Ok(
            json!({"paths":self,"workspace_id":id,"workspace_root":dunce::canonicalize(root)?,"workspace_data":directory}),
        )
    }
}
