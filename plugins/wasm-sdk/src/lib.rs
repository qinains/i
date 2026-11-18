pub use i_wasm_protocol::*;
pub use serde_json::{self, Value, json};
pub trait Plugin: Default {
    fn start(&mut self, request: ToolRequest) -> StepOutput;
    fn resume(&mut self, response: HostResponse) -> StepOutput;
}
pub mod abi;
pub struct Machine<P: Plugin> {
    plugin: P,
    started: bool,
    finished: bool,
    pending: Option<u32>,
    last_id: u32,
}
impl<P: Plugin> Default for Machine<P> {
    fn default() -> Self {
        Self { plugin: P::default(), started: false, finished: false, pending: None, last_id: 0 }
    }
}
impl<P: Plugin> Machine<P> {
    fn error(&mut self, text: &str) -> StepOutput {
        self.finished = true;
        StepOutput::Error { error: text.into() }
    }
    pub fn step(&mut self, input: StepInput) -> StepOutput {
        if self.finished {
            return self.error("invocation already finished");
        }
        let output = match input {
            StepInput::Start { request } => {
                if self.started
                    || request.event != "tool"
                    || !valid_local_name(&request.data.name)
                    || !request.data.arguments.is_object()
                    || request.id.is_empty()
                    || request.id.len() > 256
                {
                    return self.error("invalid start request");
                }
                self.started = true;
                self.plugin.start(request)
            }
            StepInput::Resume { response } => {
                if !self.started
                    || self.pending != Some(response.id)
                    || (response.ok && (response.result.is_none() || response.error.is_some()))
                    || (!response.ok && (response.error.is_none() || response.result.is_some()))
                {
                    return self.error("invalid resume response");
                }
                self.pending = None;
                self.plugin.resume(response)
            }
        };
        match &output {
            StepOutput::HostCall { request } => {
                if request.id != self.last_id + 1 || request.id > 128 || !request.args.is_object() {
                    return self.error("invalid host request or operation limit exceeded");
                }
                self.last_id = request.id;
                self.pending = Some(request.id);
            }
            StepOutput::Done { output } => {
                if output.len() > OUTPUT_LIMIT {
                    return self.error("plugin output exceeds 1 MiB");
                }
                self.finished = true;
            }
            StepOutput::Error { error } => {
                if error.len() > OUTPUT_LIMIT {
                    return self.error("plugin error exceeds 1 MiB");
                }
                self.finished = true;
            }
        }
        if serde_json::to_vec(&output).map_or(true, |v| v.len() > FRAME_LIMIT) {
            return self.error("plugin frame exceeds 16 MiB");
        }
        output
    }
}
pub fn host_call(id: u32, op: &str, args: Value) -> StepOutput {
    StepOutput::HostCall { request: HostRequest { id, op: op.into(), args } }
}
pub fn host_read(id: u32, path: &str) -> StepOutput {
    host_call(id, "fs.read", json!({"path":path}))
}
pub fn host_write(id: u32, path: &str, text: &str) -> StepOutput {
    host_call(id, "fs.write", json!({"path":path,"text":text}))
}
pub fn host_list(id: u32, path: &str) -> StepOutput {
    host_call(id, "fs.list", json!({"path":path}))
}
pub fn host_kv_get(id: u32, key: &str) -> StepOutput {
    host_call(id, "kv.get", json!({"key":key}))
}
pub fn host_kv_put(id: u32, key: &str, value: Value) -> StepOutput {
    host_call(id, "kv.put", json!({"key":key,"value":value}))
}
pub fn host_kv_delete(id: u32, key: &str) -> StepOutput {
    host_call(id, "kv.delete", json!({"key":key}))
}

#[macro_export]
macro_rules! export_plugin {
    ($plugin:ty) => {
        #[cfg(target_arch="wasm32")]
        mod harness_wasm_exports {
            std::thread_local! {static STATE: std::cell::RefCell<$crate::abi::Abi<$plugin>> = std::cell::RefCell::new($crate::abi::Abi::default());}
            #[unsafe(no_mangle)] pub extern "C" fn harness_abi_version()->u32{1}
            #[unsafe(no_mangle)] pub extern "C" fn harness_alloc(len:u32)->u32{STATE.with(|s|s.borrow_mut().alloc(len as usize).unwrap_or(0) as u32)}
            #[unsafe(no_mangle)] pub extern "C" fn harness_free(ptr:u32,len:u32){STATE.with(|s|assert!(s.borrow_mut().free(ptr as usize,len as usize),"invalid plugin free"));}
            #[unsafe(no_mangle)] pub extern "C" fn harness_step(ptr:u32,len:u32)->u64{STATE.with(|s|{let (p,n)=s.borrow_mut().step(ptr as usize,len as usize);((p as u64)<<32)|(n as u64)})}
        }
    };
}
