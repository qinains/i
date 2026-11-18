use i_wasm_sdk::*;
#[derive(Default)]
struct Counter;
impl Plugin for Counter {
    fn start(&mut self, _: ToolRequest) -> StepOutput {
        StepOutput::HostCall { request: HostRequest { id: 1, op: "fs.read".into(), args: json!({"path":"notes.txt"}) } }
    }
    fn resume(&mut self, response: HostResponse) -> StepOutput {
        StepOutput::Done { output: response.result.unwrap()["text"].as_str().unwrap().to_owned() }
    }
}
fn start() -> StepInput {
    StepInput::Start {
        request: ToolRequest {
            id: "call".into(),
            event: "tool".into(),
            workspace_id: "default".into(),
            data: ToolInvocation { name: "count".into(), arguments: json!({}) },
        },
    }
}
#[test]
fn machine_yields_resumes_and_rejects_replay() {
    let mut m = Machine::<Counter>::default();
    assert!(matches!(m.step(start()), StepOutput::HostCall { .. }));
    let response = StepInput::Resume {
        response: HostResponse { id: 1, ok: true, result: Some(json!({"text":"done"})), error: None },
    };
    assert!(matches!(m.step(response.clone()),StepOutput::Done{output} if output=="done"));
    assert!(matches!(m.step(response), StepOutput::Error { .. }));
    assert!(matches!(m.step(start()), StepOutput::Error { .. }));
}
#[test]
fn machine_rejects_mismatched_response_and_duplicate_start() {
    let mut m = Machine::<Counter>::default();
    m.step(start());
    assert!(matches!(
        m.step(StepInput::Resume { response: HostResponse { id: 2, ok: true, result: Some(json!({})), error: None } }),
        StepOutput::Error { .. }
    ));
    let mut m = Machine::<Counter>::default();
    m.step(start());
    assert!(matches!(m.step(start()), StepOutput::Error { .. }));
}
#[test]
fn abi_bounds_buffers_and_frees_each_allocation_exactly_once() {
    let mut abi = abi::Abi::<Counter>::default();
    assert!(abi.alloc(FRAME_LIMIT + 1).is_none());
    assert!(abi.alloc(0).is_none());
    let bytes = serde_json::to_vec(&start()).unwrap();
    let p = abi.alloc(bytes.len()).unwrap();
    abi.buffer_mut(p).unwrap().copy_from_slice(&bytes);
    let (q, n) = abi.step(p, bytes.len());
    assert_ne!(p, q);
    let output: StepOutput = serde_json::from_slice(abi.buffer(q).unwrap()).unwrap();
    assert!(matches!(output, StepOutput::HostCall { .. }));
    assert!(!abi.free(p, bytes.len() + 1));
    assert!(abi.free(p, bytes.len()));
    assert!(!abi.free(p, bytes.len()));
    assert!(abi.free(q, n));
    let (q, n) = abi.step(12345, 99);
    assert!(matches!(serde_json::from_slice::<StepOutput>(abi.buffer(q).unwrap()).unwrap(), StepOutput::Error { .. }));
    assert!(abi.free(q, n));
}
