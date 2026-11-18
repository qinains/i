//! Opt-in refresh and collection of superseded, confirmed client uploads only.
use super::*;
#[cfg(test)]
#[path = "provider_gc_tests.rs"]
mod tests;
fn load(path: &Path) -> Result<Value> {
    let db = if path.try_exists()? {
        serde_json::from_slice(&state::read_bounded(path, 2 * 1024 * 1024)?)?
    } else {
        json!({"version":1,"entries":{}})
    };
    ensure!(
        db["version"] == 1 && db["entries"].is_object() && db.get("retired").is_none_or(Value::is_object),
        "invalid Files registry"
    );
    Ok(db)
}
pub(super) fn prune_ready(db: &mut Value, scope: &str, config: &Value, timestamp: u64) -> bool {
    if config["auto_refresh"]["enabled"] != true || config["auto_gc"]["enabled"] != true {
        return false;
    }
    let rows = db["entries"].as_object_mut().unwrap();
    let before = rows.len();
    rows.retain(|_, r| {
        !(r["scope"] == scope
            && r["owned"] == true
            && r["status"] == "ready"
            && file_id(r).is_ok()
            && r["expires_at"].as_u64().is_some_and(|t| t.saturating_add(3600) <= timestamp))
    });
    before != rows.len()
}
pub(super) fn refresh(record: &Value, config: &Value, timestamp: u64) -> Result<bool> {
    let expiry = &record["expires_at"];
    ensure!(expiry.is_null() || expiry.is_u64(), "invalid Files expiry");
    let expired = expiry.as_u64().is_some_and(|n| n <= timestamp + 60);
    let enabled = config["auto_refresh"]["enabled"] == true;
    ensure!(!expired || enabled, "Files id expired; explicitly reconcile or enable auto_refresh");
    let aged = if let Some(age) = config["auto_refresh"]["max_age_seconds"].as_u64() {
        let created = record
            .get("cached_at")
            .unwrap_or(&record["created_at"])
            .as_u64()
            .context("missing local refresh timestamp")?;
        created.saturating_add(age) <= timestamp
    } else {
        false
    };
    Ok(enabled && (expired || aged))
}
pub(super) fn retire(db: &mut Value, record: &Value, timestamp: u64) -> Result<()> {
    if db.get("retired").is_none() {
        db["retired"] = json!({});
    }
    let retired = db["retired"].as_object_mut().context("invalid retired registry")?;
    ensure!(retired.len() < 1000, "retired Files registry full; inspect and clean explicitly");
    let mut row = record.clone();
    row["retired_at"] = json!(timestamp);
    row["status"] = json!("retired");
    retired.insert(digest(&json!([record["scope"], record["id"]])), row);
    Ok(())
}
pub(super) fn finish(data: &mut Value, account: &str, root: &Path) -> Result<()> {
    if data["operation_state"]["kind"] != "gc" {
        return Ok(());
    }
    let endpoint = endpoint(data)?;
    let scope = file_api::scope(data, &endpoint, account)?;
    let path = state::workspace_state(root, "provider-files.json")?;
    let _lock = state::Lock::acquire(&root.join("provider-files.lock"))?;
    let mut db = load(&path)?;
    let op = &data["operation_state"];
    let key = super::super::text(op, "key")?;
    let row = &db["retired"][key];
    ensure!(
        row["scope"] == scope && row["status"] == "deleting" && row["nonce"] == op["nonce"],
        "cleanup state changed; reconcile explicitly"
    );
    file_api::deleted(data, &data["operation_result"], file_id(row)?)?;
    db["retired"].as_object_mut().unwrap().remove(key);
    commit(&path, &db)?;
    data["_gc_done"] = json!(op["done"].as_u64().unwrap_or(0) + 1);
    data.as_object_mut().unwrap().remove("operation_state");
    data.as_object_mut().unwrap().remove("operation_result");
    Ok(())
}
fn candidate(db: &Value, row: &Value, scope: &str, config: &Value, timestamp: u64) -> bool {
    row["scope"] == scope
        && row["owned"] == true
        && row["status"] == "retired"
        && row["retired_at"]
            .as_u64()
            .is_some_and(|t| t.saturating_add(config["auto_gc"]["grace_seconds"].as_u64().unwrap_or(3600)) <= timestamp)
        && !db["entries"].as_object().unwrap().values().any(|r| r["id"] == row["id"] && r["status"] != "deleted")
}
pub(super) fn next(data: &Value, config: &Value, account: &str, root: &Path) -> Result<Option<Value>> {
    if config["auto_gc"]["enabled"] != true {
        return Ok(None);
    }
    ensure!(data["allow_cleanup"] == true, "auto_gc requires separate host provider_cleanup=true permission");
    if data["operations_remaining"] == 0 {
        return Ok(None);
    }
    let endpoint = endpoint(data)?;
    let scope = file_api::scope(data, &endpoint, account)?;
    fs::create_dir_all(root)?;
    let path = state::workspace_state(root, "provider-files.json")?;
    let _lock = state::Lock::acquire(&root.join("provider-files.lock"))?;
    let mut db = load(&path)?;
    let timestamp = now()?;
    let candidates: Vec<String> = db["retired"]
        .as_object()
        .into_iter()
        .flat_map(|o| o.iter())
        .filter(|(_, r)| candidate(&db, r, &scope, config, timestamp))
        .map(|(k, _)| k.clone())
        .collect();
    let mut selected = None;
    let mut pruned = prune_ready(&mut db, &scope, config, timestamp);
    for key in candidates {
        let row = &db["retired"][&key];
        file_id(row)?;
        ensure!(row.get("expires_at").is_some_and(|v| v.is_null() || v.is_u64()), "invalid retired expiry");
        if row["expires_at"].as_u64().is_some_and(|t| t.saturating_add(3600) <= timestamp) {
            db["retired"].as_object_mut().unwrap().remove(&key);
            pruned = true;
        } else if row["expires_at"].is_null()
            || row["expires_at"].as_u64().is_some_and(|t| t > timestamp.saturating_add(3600))
        {
            selected.get_or_insert(key);
        } // Let server-managed expiry settle instead of DELETE racing expiration.
    }
    if pruned {
        commit(&path, &db)?;
    }
    let Some(key) = selected else { return Ok(None) };
    let row = &mut db["retired"][&key];
    let id = file_id(row)?.to_owned();
    let nonce = uuid::Uuid::new_v4().to_string();
    row["status"] = json!("deleting");
    row["nonce"] = json!(nonce);
    commit(&path, &db)?;
    let mut operation = file_api::operation(data, &format!("{endpoint}/{id}"), "DELETE");
    operation["cleanup"] = json!(true);
    Ok(Some(
        json!({"version":1,"state":{"kind":"gc","key":key,"nonce":nonce,"done":data["_gc_done"].as_u64().unwrap_or(0),"config":digest(config)},"operation":operation}),
    ))
}
fn reason(db: &Value, row: &Value, scope: &str, config: &Value, timestamp: u64) -> &'static str {
    if row["scope"] != scope {
        return "other_scope";
    }
    if matches!(row["status"].as_str(), Some("deleting" | "pending")) {
        return "unknown_outcome";
    }
    if row["status"] != "retired" {
        return "invalid_state";
    }
    if row["owned"] != true {
        return "not_owned";
    }
    if file_id(row).is_err()
        || !row["retired_at"].is_u64()
        || !row.get("expires_at").is_some_and(|v| v.is_null() || v.is_u64())
    {
        return "invalid_metadata";
    }
    if db["entries"].as_object().unwrap().values().any(|r| r["id"] == row["id"] && r["status"] != "deleted") {
        return "referenced";
    }
    if !candidate(db, row, scope, config, timestamp) {
        return "grace_period";
    }
    match row["expires_at"].as_u64() {
        Some(t) if t.saturating_add(3600) <= timestamp => "prune_local",
        Some(t) if t <= timestamp.saturating_add(3600) => "wait_expiry",
        _ => "delete_remote",
    }
}
pub(super) fn inspect(data: Value, config: &Value, account: &str, root: &Path) -> Result<Value> {
    let action = &data["action"];
    ensure!(
        action.as_object().is_some_and(|o| o.keys().all(|k| matches!(k.as_str(), "op" | "key"))),
        "unknown GC inspection argument"
    );
    let key = super::super::text(action, "key")?;
    let endpoint = endpoint(&data)?;
    let scope = file_api::scope(&data, &endpoint, account)?;
    fs::create_dir_all(root)?;
    let path = state::workspace_state(root, "provider-files.json")?;
    let _lock = state::Lock::acquire(&root.join("provider-files.lock"))?;
    let db = load(&path)?;
    let row = &db["retired"][key];
    ensure!(row["scope"] == scope, "unknown retired key or scope mismatch");
    let id = file_id(row)?;
    let fingerprint = digest(&json!([config, action, scope, row]));
    if let Some(result) = data.get("operation_result") {
        ensure!(
            data["operation_state"]["fingerprint"] == fingerprint,
            "cleanup record/configuration changed during inspection"
        );
        let meta = file_api::metadata(&data, result)?;
        ensure!(meta["id"] == id, "Files inspection id mismatch");
        return Ok(json!({"version":1,"result":{"key":key,"local_status":row["status"],"remote":result,
            "metadata_matches_snapshot":meta["filename"]==row["filename"] && meta["bytes"]==row["bytes"],
            "local_state_changed":false,"note":"GET observes existence now, not the outcome of an earlier DELETE; no automatic replay or reset. Metadata is not proof of image content."}}));
    }
    ensure!(data["operations_remaining"].as_u64().is_some_and(|n| n > 0), "inspection operation limit exhausted");
    let mut operation = file_api::operation(&data, &format!("{endpoint}/{id}"), "GET");
    operation["headers"]["cache-control"] = json!("no-cache");
    Ok(json!({"version":1,"state":{"fingerprint":fingerprint},"operation":operation}))
}
pub(super) fn manage(mut data: Value, config: &Value, account: &str, root: &Path) -> Result<Value> {
    ensure!(
        data["action"].as_object().is_some_and(|o| o.keys().all(|k| matches!(k.as_str(), "op" | "dry_run"))),
        "unknown GC argument"
    );
    let dry = data["action"]
        .get("dry_run")
        .map(|v| v.as_bool().context("dry_run must be boolean"))
        .transpose()?
        .unwrap_or(true);
    if data.get("operation_result").is_some() {
        ensure!(data["operation_state"]["config"] == digest(config), "cleanup configuration changed");
    }
    finish(&mut data, account, root)?;
    if !dry {
        ensure!(config["auto_gc"]["enabled"] == true, "GC requires auto_gc.enabled=true");
        if let Some(plan) = next(&data, config, account, root)? {
            return Ok(plan);
        }
    }
    let endpoint = endpoint(&data)?;
    let scope = file_api::scope(&data, &endpoint, account)?;
    fs::create_dir_all(root)?;
    let path = state::workspace_state(root, "provider-files.json")?;
    let _lock = state::Lock::acquire(&root.join("provider-files.lock"))?;
    let db = load(&path)?;
    let timestamp = now()?;
    let candidates: Vec<_> = db["retired"]
        .as_object()
        .into_iter()
        .flat_map(|o| o.iter())
        .filter(|(_, r)| candidate(&db, r, &scope, config, timestamp))
        .map(|(k, r)| json!({"key":k,"id":r["id"]}))
        .collect();
    let mut counts = serde_json::Map::new();
    let records: Vec<_> = db["retired"]
        .as_object()
        .into_iter()
        .flat_map(|o| o.iter())
        .filter(|(_, r)| r["scope"] == scope)
        .map(|(key, row)| {
            let reason = reason(&db, row, &scope, config, timestamp);
            let count = counts.entry(reason).or_insert(json!(0));
            *count = json!(count.as_u64().unwrap() + 1);
            json!({"key":key,"id":row["id"],"status":row["status"],"reason":reason})
        })
        .collect();
    Ok(
        json!({"version":1,"result":{"dry_run":dry,"deleted":data["_gc_done"].as_u64().unwrap_or(0),"candidates":candidates,
        "records":records,"counts":counts,"cleanup_authorized":data["allow_cleanup"]==true,"gc_enabled":config["auto_gc"]["enabled"]==true,
        "note":"Candidates are not a deletion promise; see reason. Only confirmed remote DELETE responses count as deleted. Unknown outcomes never automatically replay or reset."}}),
    )
}
