//! Remote Files management; HTTP and destructive-method confirmation remain in the host.
use super::*;
#[cfg(test)]
#[path = "provider_manage_tests.rs"]
mod tests;
pub(super) fn handle(
    data: Value,
    args: &[String],
    paths: &AppPaths,
    workspace: &str,
    id: Option<&str>,
) -> Result<Value> {
    ensure!(data["version"] == 1, "Files management requires provider v1");
    file_api::supported(&data)?;
    ensure!(data["allow_operations"] == true, "Files management requires provider_operations=true");
    let (config, _) = configured(args)?;
    file_api::validate(&config)?;
    let account = config["account_scope"]
        .as_str()
        .filter(|s| !s.trim().is_empty() && s.len() <= 128)
        .context("Files requires account_scope")?;
    let action = &data["action"];
    let op = super::super::text(action, "op")?;
    if matches!(op, "files.gc" | "files.gc-inspect") {
        let root = paths.workspace(Path::new(workspace), id)?.1;
        return if op == "files.gc" {
            lifecycle::manage(data.clone(), &config, account, &root)
        } else {
            lifecycle::inspect(data.clone(), &config, account, &root)
        };
    }
    ensure!(
        matches!(op, "files.list" | "files.get" | "files.delete" | "files.reconcile"),
        "unknown provider management action"
    );
    let allowed: &[&str] = match op {
        "files.list" => &["op", "limit", "after", "order"],
        "files.reconcile" => &["op", "file_id", "key", "confirm"],
        _ => &["op", "file_id"],
    };
    ensure!(
        action.as_object().is_some_and(|v| v.keys().all(|k| allowed.contains(&k.as_str()))),
        "unknown management argument"
    );
    let endpoint = endpoint(&data)?;
    let scope = file_api::scope(&data, &endpoint, account)?;
    let fingerprint = digest(&json!([config, action, scope]));
    let mut url = url::Url::parse(&endpoint)?;
    let file = if op == "files.list" {
        let limit = action
            .get("limit")
            .map(|v| v.as_u64().filter(|n| (1..=1000).contains(n)).context("limit must be 1..1000"))
            .transpose()?
            .unwrap_or(100);
        let order = action.get("order").map(|v| v.as_str().context("invalid order")).transpose()?.unwrap_or("asc");
        ensure!(matches!(order, "asc" | "desc"), "order must be asc or desc");
        if data["protocol"] == "anthropic" {
            ensure!(action.get("order").is_none(), "Anthropic Files does not support order");
        } else {
            url.query_pairs_mut().append_pair("purpose", file_api::purpose(&data));
        }
        url.query_pairs_mut().append_pair("limit", &limit.to_string());
        if data["protocol"] != "anthropic" {
            url.query_pairs_mut().append_pair("order", order);
        }
        if let Some(after) = action.get("after") {
            let v = json!({"id":after});
            url.query_pairs_mut()
                .append_pair(if data["protocol"] == "anthropic" { "after_id" } else { "after" }, file_id(&v)?);
        }
        None
    } else {
        let v = json!({"id":action["file_id"]});
        let file = file_id(&v)?.to_owned();
        url.set_path(&format!("{}/{}", url.path(), file));
        Some(file)
    };
    let timestamp = now()?;
    let root = paths.workspace(Path::new(workspace), id)?.1;
    fs::create_dir_all(&root)?;
    let path = state::workspace_state(&root, "provider-files.json")?;
    let _lock = state::Lock::acquire(&root.join("provider-files.lock"))?;
    let mut db: Value = if path.try_exists()? {
        serde_json::from_slice(&state::read_bounded(&path, 2 * 1024 * 1024)?)?
    } else {
        json!({"version":1,"entries":{}})
    };
    ensure!(db["version"] == 1 && db["entries"].is_object(), "invalid Files registry");
    let retirement: Vec<_> = db["retired"]
        .as_object()
        .into_iter()
        .flat_map(|o| o.values())
        .filter(|row| file.as_deref().is_some_and(|id| row["id"] == id))
        .cloned()
        .collect();
    let key = if op == "files.reconcile" {
        ensure!(
            !retirement.iter().any(|r| r["status"] == "deleting"),
            "file has cleanup pending/unknown; reconcile cleanup first"
        );
        ensure!(
            action["confirm"] == true,
            "reconcile requires confirm=true; metadata matching is NOT proof of image content"
        );
        let key = super::super::text(action, "key")?;
        ensure!(db["entries"][key]["scope"] == scope, "unknown registry key or credential/account scope mismatch");
        Some(key)
    } else {
        None
    };
    if let Some(result) = data.get("operation_result") {
        let state = &data["operation_state"];
        ensure!(state["fingerprint"] == fingerprint, "management configuration/action changed during request");
        if op == "files.list" {
            ensure!(
                (data["protocol"] == "anthropic" || result["object"] == "list") && result["has_more"].is_boolean(),
                "invalid Files list response"
            );
            let rows = result["data"].as_array().context("missing Files list data")?;
            for row in rows {
                file_api::metadata(&data, row)?;
            }
            if result["has_more"] == true {
                ensure!(
                    !rows.is_empty() && result["last_id"] == rows.last().unwrap()["id"],
                    "invalid Files pagination cursor"
                );
            }
        } else {
            ensure!(result["id"] == file.as_deref().unwrap(), "Files response id mismatch");
            if op == "files.delete" {
                file_api::deleted(&data, result, file.as_deref().unwrap())?;
                for key in state["keys"].as_array().context("missing deletion state")? {
                    let row = &mut db["entries"][key.as_str().context("invalid deletion key")?];
                    ensure!(
                        row["scope"] == scope && row["status"] == "deleting" && row["delete_nonce"] == state["nonce"],
                        "local mapping changed during deletion; reconcile explicitly"
                    );
                    row["status"] = json!("deleted");
                    row.as_object_mut().unwrap().remove("delete_nonce");
                }
                commit(&path, &db)?;
            } else {
                let normalized = file_api::metadata(&data, result)?;
                let result = &normalized;
                if let Some(key) = key {
                    let row = &mut db["entries"][key];
                    ensure!(
                        digest(row) == state["before"] && state["retirement_guard"] == digest(&json!(retirement)),
                        "mapping/retirement state changed during reconciliation"
                    );
                    ensure!(
                        row["filename"] == result["filename"] && row["bytes"] == result["bytes"],
                        "remote filename/size do not match the local snapshot"
                    );
                    let expiry = result.get("expires_at").cloned().unwrap_or(Value::Null);
                    ensure!(
                        expiry.is_null() || expiry.as_u64().is_some_and(|t| t > timestamp.saturating_add(60)),
                        "remote file expired or near expiry"
                    );
                    ensure!(
                        !(row["ttl"].is_u64() || row["expires_at"].is_u64()) || expiry.is_u64(),
                        "cannot reconcile expiring image to a permanent file"
                    );
                    *row = json!({"scope":scope,"status":"ready","id":result["id"],"expires_at":expiry,
                        "filename":result["filename"],"bytes":result["bytes"],"created_at":result["created_at"],"cached_at":timestamp,"owned":false});
                    commit(&path, &db)?;
                }
            }
        }
        return Ok(json!({"version":1,"result":result}));
    }
    ensure!(data["operations_remaining"].as_u64().is_some_and(|n| n > 0), "management operation limit exhausted");
    let nonce = uuid::Uuid::new_v4().to_string();
    let mut state = json!({"fingerprint":fingerprint,"nonce":nonce,"keys":[]});
    if let Some(key) = key {
        state["before"] = json!(digest(&db["entries"][key]));
        state["retirement_guard"] = json!(digest(&json!(retirement)));
    }
    if op == "files.delete" {
        for (key, row) in db["entries"].as_object_mut().unwrap() {
            if row["scope"] == scope && row["id"] == file.as_deref().unwrap() {
                ensure!(
                    row["status"] != "deleted",
                    "file already marked deleted locally; query/reconcile before a new deletion"
                );
                row["status"] = json!("deleting");
                row["delete_nonce"] = json!(nonce);
                state["keys"].as_array_mut().unwrap().push(json!(key));
            }
        }
        commit(&path, &db)?;
    }
    let mut operation = file_api::operation(&data, url.as_str(), if op == "files.delete" { "DELETE" } else { "GET" });
    operation["headers"]["cache-control"] = json!("no-cache");
    Ok(json!({"version":1,"state":state,"operation":operation}))
}
