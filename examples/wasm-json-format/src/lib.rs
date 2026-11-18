use i_wasm_sdk::*;

#[derive(Default)]
pub struct JsonFormat {
    output_path: String,
    writing: bool,
}
impl Plugin for JsonFormat {
    fn start(&mut self, request: ToolRequest) -> StepOutput {
        if request.data.name != "format_json" {
            return StepOutput::Error { error: "unknown tool".into() };
        }
        let args = request.data.arguments;
        let (Some(path), Some(output_path)) = (args["path"].as_str(), args["output_path"].as_str()) else {
            return StepOutput::Error { error: "path and output_path required".into() };
        };
        if path.is_empty() || output_path.is_empty() {
            return StepOutput::Error { error: "paths cannot be empty".into() };
        }
        self.output_path = output_path.into();
        self.writing = false;
        host_read(1, path)
    }
    fn resume(&mut self, response: HostResponse) -> StepOutput {
        if !response.ok {
            return StepOutput::Error { error: response.error.unwrap_or_else(|| "host operation failed".into()) };
        }
        if self.writing {
            return StepOutput::Done { output: format!("Formatted JSON saved to {}", self.output_path) };
        }
        let result = response.result.unwrap_or(Value::Null);
        let Some(text) = result["text"].as_str() else {
            return StepOutput::Error { error: "invalid text response".into() };
        };
        let parsed: Value = match serde_json::from_str(text) {
            Ok(value) => value,
            Err(error) => return StepOutput::Error { error: format!("invalid JSON: {error}") },
        };
        let formatted = match serde_json::to_string_pretty(&parsed) {
            Ok(text) => text + "\n",
            Err(error) => return StepOutput::Error { error: error.to_string() },
        };
        self.writing = true;
        host_write(2, &self.output_path, &formatted)
    }
}
export_plugin!(crate::JsonFormat);

#[cfg(test)]
mod tests {
    use super::*;
    fn start(plugin: &mut Machine<JsonFormat>) -> StepOutput {
        plugin.step(StepInput::Start{request:serde_json::from_value(json!({"id":"fixture","event":"tool","workspace_id":"default","data":{"name":"format_json","arguments":{"path":"input.json","output_path":"formatted.json"}}})).unwrap()})
    }
    #[test]
    fn valid_json_yields_read_then_write_then_done() {
        let mut plugin = Machine::<JsonFormat>::default();
        assert!(matches!(start(&mut plugin),StepOutput::HostCall{request} if request.op=="fs.read"));
        let next = plugin.step(StepInput::Resume {
            response: HostResponse { id: 1, ok: true, result: Some(json!({"text":"{\"b\":2,\"a\":1}"})), error: None },
        });
        assert!(
            matches!(next,StepOutput::HostCall{request} if request.id==2&&request.op=="fs.write"&&request.args["path"]=="formatted.json"&&request.args["text"]=="{\n  \"a\": 1,\n  \"b\": 2\n}\n")
        );
        assert!(matches!(
            plugin.step(StepInput::Resume {
                response: HostResponse { id: 2, ok: true, result: Some(json!({"bytes":23})), error: None }
            }),
            StepOutput::Done { .. }
        ));
    }
    #[test]
    fn invalid_json_or_denied_write_never_claims_success() {
        let mut plugin = Machine::<JsonFormat>::default();
        start(&mut plugin);
        assert!(matches!(
            plugin.step(StepInput::Resume {
                response: HostResponse { id: 1, ok: true, result: Some(json!({"text":"invalid"})), error: None }
            }),
            StepOutput::Error { .. }
        ));
        let mut plugin = Machine::<JsonFormat>::default();
        start(&mut plugin);
        plugin.step(StepInput::Resume {
            response: HostResponse { id: 1, ok: true, result: Some(json!({"text":"{}"})), error: None },
        });
        assert!(matches!(
            plugin.step(StepInput::Resume {
                response: HostResponse { id: 2, ok: false, result: None, error: Some("denied".into()) }
            }),
            StepOutput::Error { .. }
        ));
    }
}
