//! Adapter for the recovered native distribution runtime.
use super::*;

pub(super) fn entry(args: Vec<String>) -> Result<Option<Vec<String>>> {
    i_plugins::entry(args.clone())?;
    Ok(Some(args))
}

pub(super) fn deployment_key(name: &str, key: String) -> Result<String> {
    i_plugins::deployment_key(name, key)
}

pub(super) fn resolve_manifest(value: Value) -> Result<Value> {
    i_plugins::resolve_manifest(value)
}

#[cfg(test)]
pub(super) fn provider(request: &Value) -> Result<Value> {
    isolated_dispatch("provider", request.clone())
}

#[cfg(test)]
pub(super) fn compact(data: &Value) -> Result<Value> {
    isolated_dispatch("compact", json!({"event":"before_compact","data":data}))
}

#[cfg(test)]
fn isolated_dispatch(module: &str, mut request: Value) -> Result<Value> {
    let home = tempfile::tempdir()?;
    request["paths"] = json!({"config_dir":home.path().join("config"),"data_dir":home.path().join("data"),"cache_dir":home.path().join("cache"),"runtime_dir":home.path().join("runtime")});
    i_plugins::dispatch(module, request, &[])
}
