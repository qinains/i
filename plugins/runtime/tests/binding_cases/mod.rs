use super::*;
use std::fs;
#[test]
fn generated_bindings_replace_every_selected_module_entry_and_respect_disable() {
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join("config");
    fs::create_dir(&config).unwrap();
    let replacement = root.path().join("custom-worker");
    let bindings = json!({"provider":{"exec":[replacement,"custom-provider"],"cwd":root.path()},
        "approval":{"exec":[replacement,"custom-approval"],"cwd":root.path()},
        "compact":{"exec":[replacement,"custom-compact"],"cwd":root.path()},
        "memory":{"exec":[replacement,"custom-memory"]}});
    fs::write(config.join("bindings.json"), bindings.to_string()).unwrap();
    let output = command(root.path()).args(["manifest", "full"]).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["provider"]["exec"], bindings["provider"]["exec"]);
    assert_eq!(value["provider"]["cwd"], bindings["provider"]["cwd"]);
    let mut count = 0;
    for section in ["tools", "hooks"] {
        for entry in value[section].as_array().unwrap() {
            match entry["module"].as_str().unwrap() {
                module @ ("compact" | "approval") => {
                    assert_eq!(entry["exec"], bindings[module]["exec"]);
                    assert_eq!(entry["cwd"], bindings[module]["cwd"]);
                    count += 1;
                }
                "memory" => {
                    assert_eq!(entry["exec"], bindings["memory"]["exec"]);
                    assert_eq!(entry["cwd"], config.to_string_lossy().as_ref());
                    count += 1;
                }
                _ => assert_ne!(entry["exec"][0], bindings["compact"]["exec"][0]),
            }
        }
    }
    assert_eq!(count, 6); // approval + compact lifecycle + memory lifecycle and three memory tools
    assert!(command(root.path()).arg("init").output().unwrap().status.success());
    let preserved: Value = serde_json::from_slice(&fs::read(config.join("bindings.json")).unwrap()).unwrap();
    assert_eq!(preserved, bindings);
    fs::write(config.join("plugins.json"), r#"{"profile":"standard","disabled":["compact"]}"#).unwrap();
    let output = command(root.path()).arg("manifest").output().unwrap();
    assert!(output.status.success());
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(!value["hooks"].as_array().unwrap().iter().any(|e| e["module"] == "compact"));
    assert!(value["hooks"].as_array().unwrap().iter().any(|e| e["module"] == "budget"));
}
#[test]
fn invalid_replacement_binding_is_an_error_not_a_builtin_fallback() {
    let root = tempfile::tempdir().unwrap();
    let config = root.path().join("config");
    fs::create_dir(&config).unwrap();
    for binding in [
        json!([]),
        json!({"compaction":{"exec":["/absolute/worker"]}}),
        json!({"compact":{"exec":[]}}),
        json!({"compact":{"exec":["relative-worker"]}}),
        json!({"compact":{"exec":[42]}}),
        json!({"compact":{"exec":[root.path().join("worker")],"cwd":"relative"}}),
        json!({"compact":{"exec":[root.path().join("worker")],"env":{"UNEXPECTED":"value"}}}),
    ] {
        fs::write(config.join("bindings.json"), binding.to_string()).unwrap();
        let output = command(root.path()).args(["manifest", "compact"]).output().unwrap();
        assert!(!output.status.success(), "accepted {binding}");
        assert!(output.stdout.is_empty());
    }
}
