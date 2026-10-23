use wasm_bindgen::prelude::*;
fn failure(error: anyhow::Error) -> JsValue {
    JsValue::from_str(&format!("{error:#}"))
}
#[wasm_bindgen]
pub fn inspect_plugin_package(bytes: &[u8]) -> Result<JsValue, JsValue> {
    let verified = super::inspect_package(bytes).map_err(failure)?;
    let manifest = serde_json::to_string(&verified.manifest).map_err(|e| failure(e.into()))?;
    let value = js_sys::JSON::parse(&manifest)?;
    let result = js_sys::Object::new();
    js_sys::Reflect::set(&result, &"manifest".into(), &value)?;
    js_sys::Reflect::set(&result, &"wasm".into(), &js_sys::Uint8Array::from(verified.wasm.as_slice()))?;
    js_sys::Reflect::set(&result, &"license".into(), &verified.license.into())?;
    js_sys::Reflect::set(&result, &"digest".into(), &verified.digest.into())?;
    Ok(result.into())
}
#[wasm_bindgen]
pub fn inspect_plugin_module(bytes: &[u8]) -> Result<(), JsValue> {
    super::inspect_module(bytes).map_err(failure)
}
#[wasm_bindgen]
pub fn inspect_plugin_manifest(json: &str) -> Result<String, JsValue> {
    let manifest: i_wasm_protocol::Manifest = serde_json::from_str(json).map_err(|e| failure(e.into()))?;
    manifest.validate().map_err(failure)?;
    serde_json::to_string(&manifest.descriptors()).map_err(|e| failure(e.into()))
}
