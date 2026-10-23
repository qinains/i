use crate::*;
use std::collections::BTreeMap;
pub struct Abi<P: Plugin> {
    buffers: BTreeMap<usize, Box<[u8]>>,
    machine: Machine<P>,
}
impl<P: Plugin> Default for Abi<P> {
    fn default() -> Self {
        Self { buffers: BTreeMap::new(), machine: Machine::default() }
    }
}
impl<P: Plugin> Abi<P> {
    pub fn alloc(&mut self, len: usize) -> Option<usize> {
        if len == 0 || len > FRAME_LIMIT || self.buffers.len() >= 4 {
            return None;
        }
        let mut bytes = vec![0; len].into_boxed_slice();
        let ptr = bytes.as_mut_ptr() as usize;
        self.buffers.insert(ptr, bytes);
        Some(ptr)
    }
    pub fn free(&mut self, ptr: usize, len: usize) -> bool {
        if self.buffers.get(&ptr).is_none_or(|v| v.len() != len) {
            return false;
        }
        self.buffers.remove(&ptr);
        true
    }
    pub fn step(&mut self, ptr: usize, len: usize) -> (usize, usize) {
        let result = match self
            .buffers
            .get(&ptr)
            .filter(|v| v.len() == len)
            .and_then(|v| serde_json::from_slice::<StepInput>(v).ok())
        {
            Some(input) => self.machine.step(input),
            None => StepOutput::Error { error: "invalid input frame or allocation".into() },
        };
        let mut output = serde_json::to_vec(&result).expect("serializable protocol output").into_boxed_slice();
        let n = output.len();
        if n > FRAME_LIMIT || self.buffers.len() >= 4 {
            return (0, 0);
        }
        let p = output.as_mut_ptr() as usize;
        self.buffers.insert(p, output);
        (p, n)
    }
    pub fn buffer_mut(&mut self, ptr: usize) -> Option<&mut [u8]> {
        self.buffers.get_mut(&ptr).map(|v| v.as_mut())
    }
    pub fn buffer(&self, ptr: usize) -> Option<&[u8]> {
        self.buffers.get(&ptr).map(|v| v.as_ref())
    }
}
