use anyhow::{Context, Result, bail, ensure};
use std::collections::BTreeMap;
use wasmparser::{Encoding, ExternalKind, Parser, Payload, ValType, Validator, WasmFeatures};
pub fn inspect_module(bytes: &[u8]) -> Result<()> {
    ensure!(bytes.len() <= 8 * 1024 * 1024, "WASM exceeds 8 MiB");
    let mut features = WasmFeatures::default();
    features.remove(
        WasmFeatures::THREADS
            | WasmFeatures::MEMORY64
            | WasmFeatures::MULTI_MEMORY
            | WasmFeatures::GC
            | WasmFeatures::EXCEPTIONS
            | WasmFeatures::CUSTOM_PAGE_SIZES,
    );
    Validator::new_with_features(features).validate_all(bytes)?;
    let mut types = Vec::new();
    let mut functions = Vec::new();
    let mut exports = BTreeMap::new();
    let mut memories = 0;
    let mut tables = 0;
    for payload in Parser::new(0).parse_all(bytes) {
        match payload? {
            Payload::Version { encoding, .. } => ensure!(encoding == Encoding::Module, "components are not supported"),
            Payload::ImportSection(reader) => ensure!(reader.count() == 0, "plugin imports are disabled"),
            Payload::StartSection { .. } => bail!("plugin start section is disabled"),
            Payload::TypeSection(reader) => {
                for ty in reader.into_iter_err_on_gc_types() {
                    types.push(ty?);
                    ensure!(types.len() <= 65536, "too many WASM types");
                }
            }
            Payload::FunctionSection(reader) => {
                for ty in reader {
                    functions.push(ty?);
                    ensure!(functions.len() <= 65536, "too many WASM functions");
                }
            }
            Payload::MemorySection(reader) => {
                for memory in reader {
                    let m = memory?;
                    memories += 1;
                    ensure!(
                        memories == 1
                            && !m.shared
                            && !m.memory64
                            && m.page_size_log2.is_none()
                            && m.initial <= 1024
                            && m.maximum.is_some_and(|n| n <= 1024),
                        "plugin memory must be a single bounded wasm32 memory <=64 MiB"
                    );
                }
            }
            Payload::TableSection(reader) => {
                for table in reader {
                    let t = table?.ty;
                    tables += 1;
                    ensure!(
                        tables <= 1
                            && !t.shared
                            && !t.table64
                            && t.initial <= 4096
                            && t.maximum.is_some_and(|n| n <= 4096),
                        "plugin table exceeds limit or has no maximum"
                    );
                }
            }
            Payload::ExportSection(reader) => {
                for export in reader {
                    let e = export?;
                    exports.insert(e.name, (e.kind, e.index));
                }
            }
            _ => {}
        }
    }
    ensure!(memories == 1 && exports.get("memory") == Some(&(ExternalKind::Memory, 0)), "plugin memory export missing");
    for (name, params, results) in [
        ("harness_abi_version", vec![], vec![ValType::I32]),
        ("harness_alloc", vec![ValType::I32], vec![ValType::I32]),
        ("harness_free", vec![ValType::I32, ValType::I32], vec![]),
        ("harness_step", vec![ValType::I32, ValType::I32], vec![ValType::I64]),
    ] {
        let (kind, index) = exports.get(name).context("plugin ABI export missing")?;
        ensure!(*kind == ExternalKind::Func, "plugin ABI export is not a function");
        let ty = types
            .get(*functions.get(*index as usize).context("invalid function index")? as usize)
            .context("invalid function type")?;
        ensure!(ty.params() == params && ty.results() == results, "plugin ABI signature mismatch: {name}");
    }
    Ok(())
}
