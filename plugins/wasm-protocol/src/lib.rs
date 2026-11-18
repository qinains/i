mod schema;
use anyhow::{Result, ensure};
pub use schema::{validate_arguments, validate_schema};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub const FRAME_LIMIT: usize = 16 * 1024 * 1024;
pub const FILE_LIMIT: usize = 8 * 1024 * 1024;
pub const OUTPUT_LIMIT: usize = 1024 * 1024;
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum WorkspaceAccess {
    None,
    Read,
    ReadWrite,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Permissions {
    pub workspace: WorkspaceAccess,
    pub storage: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tool {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub format_version: u32,
    pub id: String,
    pub name: String,
    pub version: String,
    pub runtime: String,
    pub entry: String,
    pub sha256: String,
    pub permissions: Permissions,
    pub tools: Vec<Tool>,
}
pub fn valid_id(id: &str) -> bool {
    (1..=64).contains(&id.len())
        && id.as_bytes()[0].is_ascii_lowercase()
        && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b".-".contains(&b))
}
pub fn valid_local_name(name: &str) -> bool {
    (1..=32).contains(&name.len())
        && name.as_bytes()[0].is_ascii_lowercase()
        && name.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}
pub fn valid_version(version: &str) -> bool {
    let parts: Vec<_> = version.split('.').collect();
    parts.len() == 3
        && parts.iter().all(|s| {
            !s.is_empty()
                && s.bytes().all(|b| b.is_ascii_digit())
                && (s.len() == 1 || !s.starts_with('0'))
                && s.parse::<u32>().is_ok()
        })
}
pub fn valid_digest(digest: &str) -> bool {
    digest.len() == 64 && digest.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
impl Manifest {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.format_version == 1 && self.runtime == "harness-wasm-v1" && self.entry == "plugin.wasm",
            "unsupported plugin format/runtime/entry"
        );
        ensure!(
            valid_id(&self.id) && valid_version(&self.version) && valid_digest(&self.sha256),
            "invalid plugin identity/version/digest"
        );
        ensure!(!self.name.trim().is_empty() && self.name.len() <= 256, "invalid plugin name");
        ensure!((1..=16).contains(&self.tools.len()), "plugin must declare 1..16 tools");
        let mut names = BTreeSet::new();
        for tool in &self.tools {
            ensure!(valid_local_name(&tool.name) && names.insert(&tool.name), "invalid or duplicate plugin tool name");
            ensure!(!tool.description.trim().is_empty() && tool.description.len() <= 4096, "invalid tool description");
            validate_schema(&tool.parameters)?;
        }
        ensure!(serde_json::to_vec(self)?.len() <= 64 * 1024, "plugin manifest exceeds 64 KiB");
        Ok(())
    }
    pub fn descriptors(&self) -> Vec<Descriptor> {
        self.tools
            .iter()
            .map(|tool| Descriptor {
                plugin_id: self.id.clone(),
                version: self.version.clone(),
                digest: self.sha256.clone(),
                local_name: tool.name.clone(),
                public_name: public_tool_name(&self.id, &tool.name),
                description: tool.description.clone(),
                parameters: tool.parameters.clone(),
                permissions: self.permissions.clone(),
            })
            .collect()
    }
}
pub fn validate_manifest(value: &Value) -> Result<()> {
    serde_json::from_value::<Manifest>(value.clone())?.validate()
}
pub fn public_tool_name(id: &str, name: &str) -> String {
    let hash = Sha256::digest(format!("{id}\0{name}").as_bytes());
    let hex = format!("{hash:x}");
    format!("p_{name}_{}", &hex[..16])
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Descriptor {
    pub plugin_id: String,
    pub version: String,
    pub digest: String,
    pub local_name: String,
    pub public_name: String,
    pub description: String,
    pub parameters: Value,
    pub permissions: Permissions,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolInvocation {
    pub name: String,
    pub arguments: Value,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolRequest {
    pub id: String,
    pub event: String,
    pub workspace_id: String,
    pub data: ToolInvocation,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostRequest {
    pub id: u32,
    pub op: String,
    pub args: Value,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostResponse {
    pub id: u32,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum StepInput {
    Start { request: ToolRequest },
    Resume { response: HostResponse },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum StepOutput {
    Done { output: String },
    HostCall { request: HostRequest },
    Error { error: String },
}
