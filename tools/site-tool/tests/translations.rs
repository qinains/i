use i_site_tool::translations;
use serde_json::json;
use std::{fs, path::Path};
fn fixture(root: &Path) {
    fs::create_dir_all(root.join("locales")).unwrap();
    fs::write(
        root.join("locales/languages.json"),
        json!([{"code":"en","name":"English","dir":"ltr"},{"code":"ar","name":"العربية","dir":"rtl"}]).to_string(),
    )
    .unwrap();
    for area in ["", "web", "site"] {
        let dir = root.join("locales").join(area);
        fs::create_dir_all(&dir).unwrap();
        for (code, text) in
            [("en", "Remove {item}: https://api.example/v1"), ("ar", "حذف {item}: https://api.example/v1")]
        {
            fs::write(dir.join(format!("{code}.json")), json!({"key":text}).to_string()).unwrap();
        }
    }
}
#[test]
fn deterministic_generation_and_stale_check() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    fixture(root);
    translations::generate(root, false).unwrap();
    translations::generate(root, true).unwrap();
    let before = fs::read(root.join("web/translations.js")).unwrap();
    translations::generate(root, false).unwrap();
    assert_eq!(before, fs::read(root.join("web/translations.js")).unwrap());
    fs::write(root.join("web/translations.js"), "stale").unwrap();
    assert!(translations::generate(root, true).unwrap_err().to_string().contains("stale"));
}
#[test]
fn catalog_keys_placeholders_urls_bidi_and_duplicates_fail_closed() {
    for value in [
        "{}".to_owned(),
        json!({"key":"حذف"}).to_string(),
        json!({"key":"حذف {item}: https://other.example/v1"}).to_string(),
        json!({"key":"\u{202e}حذف {item}: https://api.example/v1"}).to_string(),
        "{\"key\":\"one\",\"key\":\"two\"}".into(),
    ] {
        let temp = tempfile::tempdir().unwrap();
        fixture(temp.path());
        fs::write(temp.path().join("locales/web/ar.json"), value).unwrap();
        assert!(translations::outputs(temp.path()).is_err());
    }
}
#[test]
fn migration_preserves_all_existing_generated_content_except_generator_banner() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (name, expected) in translations::outputs(&root).unwrap() {
        let current = fs::read_to_string(root.join(&name)).unwrap();
        let content = current.split_once('\n').unwrap().1;
        let expected = expected.split_once('\n').unwrap().1;
        assert_eq!(content, expected, "{name}");
    }
}
#[test]
fn cli_generates_and_checks_a_separate_root_without_touching_served_assets() {
    let temp = tempfile::tempdir().unwrap();
    fixture(temp.path());
    let run = |arguments: &[&str]| {
        std::process::Command::new(env!("CARGO_BIN_EXE_i-site-tool"))
            .arg("--root")
            .arg(temp.path())
            .args(arguments)
            .output()
            .unwrap()
    };
    assert!(run(&["translations"]).status.success());
    assert!(run(&["translations", "--check"]).status.success());
    fs::write(temp.path().join("web/translations.js"), "stale").unwrap();
    let output = run(&["translations", "--check"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("stale"));
    assert!(!run(&["translations", "--unknown"]).status.success());
    assert!(run(&["--help"]).status.success());
}
