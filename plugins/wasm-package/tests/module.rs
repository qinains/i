use i_wasm_package::inspect_module;
fn module(memory: &str, extra: &str) -> Vec<u8> {
    wat::parse_str(format!(
        r#"(module {memory}
    (func (export "harness_abi_version") (result i32) i32.const 1)
    (func (export "harness_alloc") (param i32) (result i32) i32.const 0)
    (func (export "harness_free") (param i32 i32))
    (func (export "harness_step") (param i32 i32) (result i64) i64.const 0)
    {extra})"#
    ))
    .unwrap()
}
#[test]
fn accepts_only_bounded_import_free_core_abi() {
    assert!(inspect_module(&module("(memory (export \"memory\") 1 1024)", "")).is_ok());
    for memory in [
        "(memory (export \"memory\") 1)",
        "(memory (export \"memory\") 1 1025)",
        "(memory (export \"memory\") 1 1024 shared)",
        "(memory (export \"memory\") i64 1 10)",
        "(memory (export \"memory\") 1 10) (memory 1 10)",
    ] {
        assert!(inspect_module(&module(memory, "")).is_err(), "accepted {memory}");
    }
    for extra in ["(func $start) (start $start)", "(table 1 funcref)", "(table 1 4097 funcref)"] {
        assert!(inspect_module(&module("(memory (export \"memory\") 1 1024)", extra)).is_err());
    }
    assert!(inspect_module(&wat::parse_str("(module (import \"env\" \"f\" (func)))").unwrap()).is_err());
    assert!(inspect_module(&wat::parse_str("(module (memory 1 10))").unwrap()).is_err());
    assert!(inspect_module(b"not wasm").is_err());
}
#[test]
fn validates_function_bodies_and_required_export_signatures() {
    let bytes = module("(memory (export \"memory\") 1 10)", "");
    let mut wrong = bytes.clone();
    *wrong.last_mut().unwrap() = 0xff;
    assert!(inspect_module(&wrong).is_err());
    let wrong = wat::parse_str(
        r#"(module (memory (export "memory") 1 10) (func (export "harness_step") (result i32) i32.const 1))"#,
    )
    .unwrap();
    assert!(inspect_module(&wrong).is_err());
}
#[test]
fn emit_browser_fixtures() {
    let Some(path) = std::env::var_os("HARNESS_PLUGIN_FIXTURES") else { return };
    let dir = std::path::Path::new(&path);
    std::fs::create_dir_all(dir).unwrap();
    for (name, body) in [
        ("loop", "(loop br 0) i64.const 0"),
        ("abi-loop", "i64.const 0"),
        ("bad-pointer", "i64.const 0"),
        ("trap", "unreachable"),
    ] {
        let version = if name == "abi-loop" { "(loop br 0) i32.const 1" } else { "i32.const 1" };
        let wat = format!(
            r#"(module (memory (export "memory") 1 1024)
          (func (export "harness_abi_version") (result i32) {version})
          (func (export "harness_alloc") (param i32) (result i32) i32.const 4096)
          (func (export "harness_free") (param i32 i32))
          (func (export "harness_step") (param i32 i32) (result i64) {body}))"#
        );
        let wasm = wat::parse_str(wat).unwrap();
        inspect_module(&wasm).unwrap();
        std::fs::write(dir.join(format!("{name}.wasm")), wasm).unwrap();
    }
    for (name, output) in [
        (
            "repeated-host",
            serde_json::json!({"type":"host_call","request":{"id":1,"op":"fs.read","args":{"path":"notes.txt"}}}),
        ),
        ("oversized-output", serde_json::json!({"type":"done","output":"x".repeat(1024*1024+1)})),
    ] {
        let text = output.to_string();
        let data = text.bytes().map(|b| format!("\\{b:02x}")).collect::<String>();
        let packed = (128u64 << 32) | text.len() as u64;
        let wat = format!(
            r#"(module (memory (export "memory") 64 1024) (data (i32.const 128) "{data}")
          (func (export "harness_abi_version") (result i32) i32.const 1)
          (func (export "harness_alloc") (param i32) (result i32) i32.const 2097152)
          (func (export "harness_free") (param i32 i32))
          (func (export "harness_step") (param i32 i32) (result i64) i64.const {packed}))"#
        );
        let wasm = wat::parse_str(wat).unwrap();
        inspect_module(&wasm).unwrap();
        std::fs::write(dir.join(format!("{name}.wasm")), wasm).unwrap();
    }
    let write=serde_json::json!({"type":"host_call","request":{"id":1,"op":"fs.write","args":{"path":"/workspace/plugin-result.txt","text":"committed plugin write"}}}).to_string();
    let done = serde_json::json!({"type":"done","output":"written"}).to_string();
    let payload = format!("{write}{done}").bytes().map(|b| format!("\\{b:02x}")).collect::<String>();
    let first = (128u64 << 32) | write.len() as u64;
    let second = ((128 + write.len() as u64) << 32) | done.len() as u64;
    let wat = format!(
        r#"(module (memory (export "memory") 1 1024) (global $called (mut i32) (i32.const 0)) (data (i32.const 128) "{payload}")
      (func (export "harness_abi_version") (result i32) i32.const 1)
      (func (export "harness_alloc") (param i32) (result i32) i32.const 4096)
      (func (export "harness_free") (param i32 i32))
      (func (export "harness_step") (param i32 i32) (result i64) global.get $called if (result i64) i64.const {second} else i32.const 1 global.set $called i64.const {first} end))"#
    );
    let wasm = wat::parse_str(wat).unwrap();
    inspect_module(&wasm).unwrap();
    std::fs::write(dir.join("write-once.wasm"), wasm).unwrap();
}
