use super::*;

pub(super) fn entries(manifest: &Value) -> Result<Vec<Value>> {
    let entries = manifest.get("analyses").cloned().unwrap_or(json!([]));
    let entries = entries.as_array().context("invalid analyses")?;
    ensure!(entries.len() <= 32, "at most 32 analysis plugins");
    let mut names = Vec::new();
    for entry in entries {
        let name = field(entry, "name")?;
        ensure!(valid_name(name) && !names.contains(&name), "invalid/duplicate analysis name");
        names.push(name);
        ensure!(
            entry.as_object().is_some_and(|entry| entry
                .keys()
                .all(|key| matches!(key.as_str(), "name" | "selection" | "exec" | "cwd" | "module"))),
            "invalid analysis entry fields"
        );
        let selection: i::analysis::Selection = serde_json::from_value(entry["selection"].clone())?;
        selection.validate()?;
        let exec: Vec<String> = serde_json::from_value(entry["exec"].clone())?;
        ensure!(!exec.is_empty() && exec.iter().all(|part| !part.is_empty()), "invalid analysis worker");
        ensure!(entry.get("cwd").is_none_or(Value::is_string), "invalid analysis cwd");
    }
    Ok(entries.clone())
}
impl Api {
    pub(super) async fn analyze(&self, name: &str, history: &[Value], roles: &[String]) -> Result<String> {
        let entry = self
            .plugins
            .analyses
            .iter()
            .find(|entry| entry["name"] == name)
            .context("analysis plugin is not installed")?;
        let mut selection: i::analysis::Selection = serde_json::from_value(entry["selection"].clone())?;
        if !roles.is_empty() {
            ensure!(
                roles.iter().all(|role| selection.roles.contains(role)),
                "analysis cannot widen declared conversation access"
            );
            selection.roles = roles.to_vec();
        }
        let transcript = i::analysis::project(history, &selection)?;
        let data = self.plugins.call(
            entry,
            "analysis_prepare",
            json!({"name":name,"transcript":transcript,"model":self.model}),
        )?;
        let summary = if let Some(summary) = data.get("summary") {
            summary.as_str().context("invalid analysis summary")?.to_owned()
        } else {
            let input = i::analysis::request(field(&data, "prompt")?, &transcript, &json!([]))?;
            i::analysis::response_text(&self.complete(&input, true).await?)?
        };
        i::analysis::validate_summary(&summary)?;
        let result = self.plugins.call(entry, "analysis_result", json!({"summary":summary,"state":data["state"]}))?;
        let summary = field(&result, "summary")?;
        i::analysis::validate_summary(summary)?;
        Ok(summary.to_owned())
    }
}
