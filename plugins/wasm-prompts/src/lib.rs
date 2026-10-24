use i_wasm_sdk::*;

#[derive(Default)]
pub struct Prompts;
impl Plugin for Prompts {
    fn start(&mut self, request: ToolRequest) -> StepOutput {
        let args = &request.data.arguments;
        match args["action"].as_str() {
            Some("get") => host_kv_get(1, "system"),
            Some("reset") => host_kv_delete(1, "system"),
            Some("set") => match args["prompt"].as_str() {
                Some(prompt) if !prompt.trim().is_empty() && prompt.len() <= 131072 => {
                    host_kv_put(1, "system", json!(prompt))
                }
                _ => StepOutput::Error { error: "prompt must contain 1-131072 UTF-8 bytes".into() },
            },
            _ => StepOutput::Error { error: "action must be get, set or reset".into() },
        }
    }
    fn resume(&mut self, response: HostResponse) -> StepOutput {
        if !response.ok {
            return StepOutput::Error { error: response.error.unwrap_or_else(|| "prompt operation failed".into()) };
        }
        StepOutput::Done { output: json!({"result":response.result,"note":"Changes take effect on the next user task. Null means the built-in system prompt."}).to_string() }
    }
}
export_plugin!(crate::Prompts);
