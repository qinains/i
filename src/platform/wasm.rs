use super::*;
use anyhow::{Context, anyhow, bail};
use js_sys::{Function, Promise, Uint8Array};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

#[wasm_bindgen(module = "/web/platform.js")]
extern "C" {
    #[wasm_bindgen(catch, js_name = opfsOpen)]
    async fn open() -> std::result::Result<JsValue, JsValue>;
    #[wasm_bindgen(catch, js_name = opfsRead)]
    async fn read(root: &JsValue, path: &str, limit: usize) -> std::result::Result<JsValue, JsValue>;
    #[wasm_bindgen(catch, js_name = opfsWrite)]
    async fn write(root: &JsValue, path: &str, bytes: &[u8]) -> std::result::Result<JsValue, JsValue>;
    #[wasm_bindgen(catch, js_name = opfsMkdir)]
    async fn mkdir(root: &JsValue, path: &str) -> std::result::Result<JsValue, JsValue>;
    #[wasm_bindgen(catch, js_name = opfsRemove)]
    async fn remove(root: &JsValue, path: &str) -> std::result::Result<JsValue, JsValue>;
    #[wasm_bindgen(catch, js_name = opfsList)]
    async fn list(root: &JsValue, path: &str) -> std::result::Result<JsValue, JsValue>;
    #[wasm_bindgen(catch, js_name = opfsMetadata)]
    async fn metadata(root: &JsValue, path: &str) -> std::result::Result<JsValue, JsValue>;
}
pub fn js_error(error: JsValue) -> anyhow::Error {
    let message = js_sys::Reflect::get(&error, &"message".into()).ok().and_then(|v| v.as_string());
    anyhow!("{}", message.or_else(|| error.as_string()).unwrap_or_else(|| format!("{error:?}")))
}
pub fn from_js<T: serde::de::DeserializeOwned>(value: &JsValue) -> Result<T> {
    let text = js_sys::JSON::stringify(value).map_err(js_error)?.as_string().context("invalid JSON value")?;
    Ok(serde_json::from_str(&text)?)
}
pub fn to_js(value: &serde_json::Value) -> Result<JsValue> {
    js_sys::JSON::parse(&value.to_string()).map_err(js_error)
}
fn name(path: &Path) -> Result<&str> {
    path.to_str().context("non-Unicode path")
}

pub struct OpfsFileSystem {
    root: JsValue,
}
impl OpfsFileSystem {
    pub async fn open() -> Result<Self> {
        Ok(Self { root: open().await.map_err(js_error)? })
    }
}
impl FileSystem for OpfsFileSystem {
    async fn read_file_limited(&self, path: &Path, limit: usize) -> Result<Vec<u8>> {
        Ok(Uint8Array::new(&read(&self.root, name(path)?, limit).await.map_err(js_error)?).to_vec())
    }
    async fn write_file(&self, path: &Path, bytes: &[u8]) -> Result<()> {
        write(&self.root, name(path)?, bytes).await.map_err(js_error)?;
        Ok(())
    }
    async fn create_dir_all(&self, path: &Path) -> Result<()> {
        mkdir(&self.root, name(path)?).await.map_err(js_error)?;
        Ok(())
    }
    async fn remove_file(&self, path: &Path) -> Result<()> {
        remove(&self.root, name(path)?).await.map_err(js_error)?;
        Ok(())
    }
    async fn read_dir(&self, path: &Path) -> Result<Vec<PathBuf>> {
        let paths: Vec<String> = from_js(&list(&self.root, name(path)?).await.map_err(js_error)?)?;
        paths.iter().map(|p| crate::workspace::path::validate(p, false)).collect()
    }
    async fn metadata(&self, path: &Path) -> Result<Option<FileMetadata>> {
        from_js(&metadata(&self.root, name(path)?).await.map_err(js_error)?)
    }
    async fn canonicalize(&self, path: &Path) -> Result<PathBuf> {
        let p = crate::workspace::path::validate(name(path)?, true)?;
        anyhow::ensure!(self.metadata(&p).await?.is_some(), "path not found");
        Ok(p)
    }
}
#[derive(Clone)]
pub struct WebTerminal {
    pub event: Function,
    pub approval: Function,
}
impl Terminal for WebTerminal {
    async fn read_line(&self, _: &str) -> Result<String> {
        bail!("use the task input to submit a new task")
    }
    fn write(&self, text: &str) -> Result<()> {
        self.event
            .call1(&JsValue::NULL, &to_js(&serde_json::json!({"type":"assistant","text":text}))?)
            .map_err(js_error)?;
        Ok(())
    }
    async fn approve(&self, description: &str) -> Result<bool> {
        let result = self.approval.call1(&JsValue::NULL, &description.into()).map_err(js_error)?;
        Ok(JsFuture::from(Promise::resolve(&result)).await.map_err(js_error)?.as_bool().unwrap_or(false))
    }
}
pub struct LimitedProcess;
impl ProcessRunner for LimitedProcess {
    async fn execute(&self, _: &str, _: &[&str]) -> Result<ProcessOutput> {
        bail!("process execution is disabled in browser")
    }
    fn is_supported(&self) -> bool {
        false
    }
}
pub struct WasmPlatform {
    pub fs: OpfsFileSystem,
    pub terminal: WebTerminal,
}
impl Platform for WasmPlatform {
    type FileSystem = OpfsFileSystem;
    type Terminal = WebTerminal;
    type Process = LimitedProcess;
    fn fs(&self) -> &OpfsFileSystem {
        &self.fs
    }
    fn terminal(&self) -> &WebTerminal {
        &self.terminal
    }
    fn process(&self) -> &LimitedProcess {
        &LimitedProcess
    }
    fn capabilities(&self) -> Capabilities {
        Capabilities::BROWSER
    }
}
