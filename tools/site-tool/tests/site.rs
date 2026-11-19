use i_site_tool::{copy_tree, cors, hash, site, translations, write};
use regex::Regex;
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};
fn source() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap()
}
fn fixture(root: &Path) {
    let source = source();
    for directory in ["locales", "site", "docs", "extensions"] {
        copy_tree(&source.join(directory), &root.join(directory)).unwrap();
    }
    for name in
        ["README.md", "README.zh-CN.md", "CONTRIBUTING.md", "SECURITY.md", "LICENSE", "THIRD_PARTY_LICENSES.txt"]
    {
        write(&root.join(name), fs::read(source.join(name)).unwrap()).unwrap();
    }
    translations::generate(root, false).unwrap();
    cors::build(root).unwrap();
    for name in ["index.html", "app.js"] {
        write(&root.join("web").join(name), fs::read(source.join("web").join(name)).unwrap()).unwrap();
    }
    write(&root.join("web/THIRD_PARTY_LICENSES.txt"), fs::read(source.join("THIRD_PARTY_LICENSES.txt")).unwrap())
        .unwrap();
    write(&root.join("web/pkg/i_bg.wasm"), b"\0asm\x01\0\0\0").unwrap();
    write(&root.join("web/tests/private.txt"), b"must not publish").unwrap();
    write(&root.join("web/node_modules/private.txt"), b"must not publish").unwrap();
    let assets =
        ["index.html", "app.js", "languages.js", "translations.js", "THIRD_PARTY_LICENSES.txt", "pkg/i_bg.wasm"];
    let precache = assets
        .iter()
        .map(|name| json!({"url":name,"sha256":hash(&fs::read(root.join("web").join(name)).unwrap())}))
        .collect::<Vec<_>>();
    write(&root.join("web/assets.json"), json!({"version":"test","assets":assets,"precache":precache}).to_string())
        .unwrap();
}
#[test]
fn native_downloads_reject_unverified_shapes() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    write(&root.join("site/install.html"), "{{NATIVE_CLIENTS}}").unwrap();
    let client = json!({"platform":"macos","target":"aarch64-apple-darwin","file":"i-0.1.0-aarch64-apple-darwin-min-size.tar.gz","sha256":"a".repeat(64)});
    let render = |version: Value, clients: Value| {
        write(&root.join("site/releases.json"), json!({"version":version,"clients":clients}).to_string()).unwrap();
        site::render_client_install(root)
    };
    assert!(!render(Value::Null, json!([])).unwrap().contains("/releases/download/"));
    let output = render(json!("0.1.0"), json!([client])).unwrap();
    for text in [
        "/releases/download/v0.1.0/i-0.1.0-aarch64-apple-darwin-min-size.tar.gz",
        ".tar.gz.sha256",
        "<code>./bin/i</code>",
        "<code>.\\bin\\i.exe</code>",
        "data-native-install",
    ] {
        assert!(output.contains(text), "{text}");
    }
    for text in ["<code>i setup</code>", "i start -C ."] {
        assert!(!output.contains(text));
    }
    for (key, value) in
        [("platform", "unknown"), ("file", "../secret"), ("target", "universal"), ("sha256", "not-a-checksum")]
    {
        let mut bad = client.clone();
        bad[key] = value.into();
        assert!(render(json!("0.1.0"), json!([bad])).is_err());
    }
    assert!(render(json!("../bad"), json!([client])).is_err());
    assert!(render(json!("0.1.0"), json!([client, client])).is_err());

    let dmg_file = "i-0.1.0-macos-arm64-developer-id.dmg";
    let mut dmg = client.clone();
    dmg["file"] = dmg_file.into();
    let output = render(json!("0.1.0"), json!([dmg])).unwrap();
    let mac_panel = output.split_once("id=\"client-macos\"").unwrap().1.split_once("id=\"client-windows\"").unwrap().0;
    let url = format!("/releases/download/v0.1.0/{dmg_file}");
    for text in [
        &format!("{url}\""),
        &format!("{url}.sha256\""),
        "NATIVE CLI / DMG",
        "install.macos.dmg.note",
        "install.macos.dmg.step1.detail",
        "<code>\"/Volumes/i 0.1.0/bin/i\"</code>",
    ] {
        assert!(mac_panel.contains(text), "{text}");
    }
    assert!(!mac_panel.contains("install.macos.note"));
    assert!(!mac_panel.contains("TAR.GZ"));
    assert!(render(json!("0.1.1"), json!([dmg])).is_err());
    dmg["platform"] = "linux".into();
    dmg["target"] = "x86_64-unknown-linux-gnu".into();
    assert!(render(json!("0.1.0"), json!([dmg])).is_err());
}
#[test]
fn markdown_escapes_html_preserves_blocks_and_routes_safe_links() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    write(&root.join("docs/index.md"), "# Docs").unwrap();
    let source = root.join("docs/example.md");
    let (rendered, toc) = site::markdown(
        root,
        "## Same\n## Same\n\n<script>alert(1)</script> [Docs](index.md#part) [bad](javascript:alert) [escape](../../secret) ![alt](image.png) **bold** `code`\n> **Note**\n> <escaped>\n\n```rs\n<x>\n```\n\n| A | B |\n| -- | -- |\n| one | two |\n\n1. first\n2. second\n\n---",
        &source,
    );
    for text in [
        "id=\"same\"",
        "id=\"same-2\"",
        "&lt;script&gt;",
        "href=\"/docs/#part\"",
        "href=\"#\"",
        "alt",
        "<strong>bold</strong>",
        "<code>code</code>",
        "<blockquote><p><strong>Note</strong> &lt;escaped&gt;</p></blockquote>",
        "<span>rs</span>",
        "&lt;x&gt;",
        "<th>A</th>",
        "<td>one</td>",
        "<ol><li>first</li><li>second</li></ol>",
        "<hr>",
    ] {
        assert!(rendered.contains(text), "{text}: {rendered}");
    }
    assert_eq!(toc.len(), 2);
    assert!(!rendered.contains("href=\"javascript:"));
    assert_eq!(site::href(root, "//evil.test", &source), "#");
    assert_eq!(site::href(root, "mailto:person@example.test", &source), "mailto:person@example.test");
    assert_eq!(site::href(root, "missing.js", &source), "https://github.com/qinains/i/blob/main/docs/missing.js");
}
#[test]
fn localized_sign_in_guide_preserves_commands_and_links() {
    let root = source();
    let languages: Value = serde_json::from_slice(&fs::read(root.join("locales/languages.json")).unwrap()).unwrap();
    let baseline = fs::read_to_string(root.join("docs/translations/en/chatgpt-login.md")).unwrap();
    let original = fs::read_to_string(root.join("docs/chatgpt-login.md")).unwrap();
    let fences = Regex::new(r"(?s)```[^\n]*\n(.*?)\n```").unwrap();
    let comments = Regex::new(r"(?m) +# [^\n]+").unwrap();
    let spans = Regex::new(r"`([^`\n]+)`").unwrap();
    let links = Regex::new(r"\]\((https://[^\s)]+)\)").unwrap();
    let signature = |text: &str| {
        let commands = fences
            .captures_iter(text)
            .map(|block| comments.replace_all(&block[1], "").into_owned())
            .collect::<Vec<_>>();
        let prose = fences.replace_all(text, "");
        let collect = |pattern: &Regex| {
            let mut values = pattern.captures_iter(&prose).map(|part| part[1].to_owned()).collect::<Vec<_>>();
            values.sort_unstable();
            values
        };
        (commands, collect(&spans), collect(&links))
    };
    for locale in languages.as_array().unwrap() {
        let code = locale["code"].as_str().unwrap();
        if matches!(code, "en" | "zh-CN") {
            continue;
        }
        let localized = fs::read_to_string(root.join(format!("docs/translations/{code}/chatgpt-login.md"))).unwrap();
        let reference = if code == "zh-TW" { &original } else { &baseline };
        assert_eq!(signature(&localized), signature(reference), "{code}");
    }
}
#[test]
fn full_site_preserves_public_pages_localization_security_and_asset_allowlist() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    fixture(root);
    write(&root.join("web/downloads/private-canary.json"), "private").unwrap();
    let output = root.join("dist/site");
    let count = site::build(root, "https://i.lninl.com", &output).unwrap();
    assert!(count > 20);
    let login = fs::read_to_string(output.join("docs/chatgpt-login/index.html")).unwrap();
    assert!(login.contains("<article lang=\"zh-CN\""));
    let languages: Value = serde_json::from_slice(&fs::read(root.join("locales/languages.json")).unwrap()).unwrap();
    for locale in languages.as_array().unwrap() {
        let code = locale["code"].as_str().unwrap();
        let dir = locale["dir"].as_str().unwrap();
        assert!(login.contains(&format!("<template data-docs-variant=\"{code}\" data-docs-dir=\"{dir}\"")), "{code}");
    }
    assert!(login.contains("Manage sign-in plugins with i"));
    assert!(login.contains("href=\"#浏览器配对\" lang=\"en\" dir=\"auto\">Pair with a browser</a>"));
    assert!(!output.join("docs/translations").exists());
    assert_eq!(
        fs::read(output.join("third-party-licenses.txt")).unwrap(),
        fs::read(root.join("THIRD_PARTY_LICENSES.txt")).unwrap()
    );
    for name in [
        "index.html",
        "docs/index.html",
        "docs/plugin-development/index.html",
        "docs/wasm-plugin-reference/index.html",
        "sitemap.xml",
        "search-index.json",
        "app/pkg/i_bg.wasm",
        "app/THIRD_PARTY_LICENSES.txt",
    ] {
        assert!(output.join(name).is_file(), "{name}");
    }
    for name in ["app/tests", "app/node_modules", "docs/superpowers", "app/app.js.map", "downloads/private-canary.json"]
    {
        assert!(!output.join(name).exists(), "{name}");
    }
    let home = fs::read_to_string(output.join("index.html")).unwrap();
    let plugins = fs::read_to_string(output.join("plugins/index.html")).unwrap();
    for text in [
        "data-page=\"plugins\"",
        "connect-src 'self' https://qinains.github.io;",
        "/app/?settings=plugins",
        "https://github.com/qinains/i-plugins",
        "id=\"native-plugins\"",
        "id=\"browser-plugins\"",
        "verify_project_check",
        "href=\"/docs/plugin-development/#示例-用验收证据核对浏览器会话恢复\"",
    ] {
        assert!(plugins.contains(text), "{text}");
    }
    assert!(fs::read_to_string(output.join("sitemap.xml")).unwrap().contains("https://i.lninl.com/plugins/"));
    assert!(
        fs::read_to_string(output.join("docs/plugin-development/index.html"))
            .unwrap()
            .contains("id=\"示例-用验收证据核对浏览器会话恢复\"")
    );
    assert!(home.contains("href=\"/plugins/\" data-site-i18n=\"nav.plugins\">插件</a>"));
    for text in [
        "href=\"#security\"",
        "id=\"security\"",
        "data-site-i18n=\"boundary.data.detail\"",
        "data-site-i18n=\"boundary.credentials.detail\"",
        "data-site-i18n=\"boundary.plugins.detail\"",
        "data-site-i18n=\"boundary.preview.detail\"",
        "href=\"/docs/connect-model/\"",
        "class=\"launch-card\"",
        "描述想法，",
        "少一点配置，多一点创造",
        "跟随引导，完成首次配置",
        "value=\"ar\" lang=\"ar\" dir=\"rtl\"",
        "https://www.producthunt.com/products/i-lninl?embed=true&amp;utm_source=embed&amp;utm_medium=post_embed",
    ] {
        assert!(home.contains(text), "{text}");
    }
    for text in ["{{", "首次自动配置，然后开始", "ph-files.imgix.net"] {
        assert!(!home.contains(text), "{text}");
    }
    for platform in ["macos", "windows", "linux", "browser"] {
        assert!(home.contains(&format!("id=\"client-{platform}\"")));
    }
    let security = fs::read_to_string(output.join("docs/security/index.html")).unwrap();
    for boundary in ["localStorage", "API key", "debugger", "same-origin", "not encrypted"] {
        assert!(security.contains(boundary), "{boundary}");
    }
    let guide = fs::read_to_string(output.join("docs/connect-model/index.html")).unwrap();
    for text in [
        "https://platform.deepseek.com/",
        "deepseek-flash",
        "https://platform.openai.com/api-keys",
        "https://platform.claude.com/settings/keys",
        "qwen-plus",
        "CORS",
        "401",
    ] {
        assert!(guide.contains(text), "{text}");
    }
    let tutorial = fs::read_to_string(output.join("docs/plugin-development/index.html")).unwrap();
    for text in [
        "<article lang=\"zh-CN\" dir=\"ltr\">",
        "data-docs-source=\"zh-CN\"",
        "data-site-i18n=\"docs.edit\"",
        "build_wasm_plugin.sh",
        "hplugin.zip",
        "https://i.lninl.com/docs/plugin-development/",
    ] {
        assert!(tutorial.contains(text), "{text}");
    }
    assert!(!tutorial.contains("href=\"javascript:"));
    for name in ["docs/overview/index.html", "license/index.html"] {
        assert!(fs::read_to_string(output.join(name)).unwrap().contains("<article lang=\"en\" dir=\"ltr\">"));
    }
    let docs_index = fs::read_to_string(output.join("docs/index.html")).unwrap();
    for marker in [
        "/assets/docs.css",
        "data-docs-title",
        "data-site-i18n=\"docs.intro\"",
        "data-site-i18n=\"docs.navigation\"",
        "data-site-i18n=\"docs.toc\"",
        "href=\"#运维-边界与贡献\"",
        "id=\"运维-边界与贡献\"",
        "href=\"/docs/plugin-publishing/\" data-site-i18n=\"docs.page.plugin-publishing\"",
    ] {
        assert!(docs_index.contains(marker), "{marker}");
    }
    assert!(!docs_index.contains("data-docs-source"));
    site::build(root, "https://i.lninl.com", &output).unwrap();
    fs::write(root.join("web/app.js"), "stale").unwrap();
    assert!(
        site::build(root, "https://i.lninl.com", &output).unwrap_err().to_string().contains("Stale application build")
    );
    assert!(output.join(".harness-site-build").is_file());
    let unrecognized = root.join("unrecognized");
    fs::create_dir(&unrecognized).unwrap();
    assert!(site::build(root, "https://i.lninl.com", &unrecognized).unwrap_err().to_string().contains("unrecognized"));
    assert!(site::build(root, "http://i.lninl.com", &root.join("insecure")).is_err());
}
