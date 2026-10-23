use anyhow::{Context, Result, ensure};
use i_wasm_protocol::{
    Descriptor, public_tool_name, valid_digest, valid_id, valid_local_name, valid_version, validate_arguments,
    validate_schema,
};
use serde_json::{Value, json};
use std::collections::BTreeSet;
#[derive(Clone, Default)]
pub struct PluginRegistry {
    tools: Vec<Descriptor>,
}
impl PluginRegistry {
    pub fn from_json(text: &str) -> Result<Self> {
        ensure!(text.len() <= 1024 * 1024, "plugin registry exceeds 1 MiB");
        let tools: Vec<Descriptor> = serde_json::from_str(text)?;
        ensure!(tools.len() <= 128, "plugin registry exceeds 128 tools");
        let mut names = BTreeSet::new();
        for d in &tools {
            ensure!(
                valid_id(&d.plugin_id)
                    && valid_version(&d.version)
                    && valid_digest(&d.digest)
                    && valid_local_name(&d.local_name),
                "invalid plugin descriptor"
            );
            ensure!(
                d.public_name == public_tool_name(&d.plugin_id, &d.local_name) && names.insert(&d.public_name),
                "invalid/duplicate plugin public name"
            );
            ensure!(!d.description.trim().is_empty() && d.description.len() <= 4096, "invalid plugin description");
            validate_schema(&d.parameters)?;
        }
        Ok(Self { tools })
    }
    pub fn find(&self, name: &str) -> Option<&Descriptor> {
        self.tools.iter().find(|d| d.public_name == name)
    }
    pub fn validate_call(&self, name: &str, args: &Value) -> Result<&Descriptor> {
        let d = self.find(name).context("plugin is unavailable or not enabled in this task")?;
        validate_arguments(&d.parameters, args)?;
        Ok(d)
    }
    pub fn schemas(&self) -> Vec<Value> {
        self.tools.iter().map(|d|json!({"type":"function","function":{"name":d.public_name,"description":d.description,"parameters":d.parameters}})).collect()
    }
}
