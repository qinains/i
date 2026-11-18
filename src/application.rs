use crate::{
    platform::{Capabilities, FileSystem},
    workspace::{Workspace, tool_schemas},
};
use anyhow::Result;
use serde_json::Value;

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(not(target_arch = "wasm32"))]
pub use native::{AgentApplication, ApplicationHost, Conversation, Services};

/// Domain operations only; the host's composition kernel owns platform resources.
#[derive(Default)]
pub struct WorkspaceTools;
impl WorkspaceTools {
    pub fn schemas(&self, capabilities: Capabilities) -> Value {
        tool_schemas(capabilities)
    }
    pub async fn execute<F: FileSystem>(&self, workspace: &Workspace<F>, name: &str, args: &Value) -> Result<Value> {
        workspace.execute(name, args).await
    }
}
