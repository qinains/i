use crate::{copy_tree, hash, read_json, text, write};
use anyhow::{Context, Result, ensure};
use regex::{Captures, Regex};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path, PathBuf},
    sync::LazyLock,
};
static SCHEME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^([A-Za-z][A-Za-z0-9+.-]*):").unwrap());
static INLINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"`[^`]+`|!\[[^\]]*\]\([^)]+\)|\[[^\]]+\]\([^)]+\)|\*\*[^*]+\*\*").unwrap());
static LINK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^!?\[([^\]]*)\]\(([^)]+)\)$").unwrap());
static SHA: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[a-f0-9]{64}$").unwrap());
const REPOSITORY: &str = "https://github.com/qinains/i";
const EXCLUDED: &[&str] = &[
    "tools-composer-verification.md",
    "dependency-size-history.md",
    "dependency-audit.md",
    "dependency-size.md",
    "size-report.md",
];
const FEATURED: &[&str] = &[
    "index",
    "connect-model",
    "system-overview",
    "plugin-development",
    "plugin-publishing",
    "wasm-plugin-reference",
    "plugins",
    "wasm",
    "deployment",
    "providers",
    "file-tools",
    "cors-extension",
    "site-deployment",
    "opfs-concurrency",
];
const ENGLISH: &[&str] = &[
    "README.md",
    "CONTRIBUTING.md",
    "SECURITY.md",
    "LICENSE",
    "compaction.md",
    "console.md",
    "composer.md",
    "file-tools.md",
    "opfs-concurrency.md",
    "network.md",
    "wasm-size.md",
];
const DOC_GROUPS: &[(&str, &str, &[&str])] = &[
    ("start", "开始使用", &["connect-model", "system-overview", "wasm", "deployment", "console", "file-tools"]),
    (
        "develop",
        "为系统开发插件",
        &[
            "plugin-development",
            "plugin-publishing",
            "wasm-plugin-reference",
            "plugins",
            "conversation-analysis",
            "providers",
            "approval",
            "compaction",
            "cache",
        ],
    ),
    (
        "maintain",
        "运维-边界与贡献",
        &["opfs-concurrency", "cors-extension", "site-deployment", "contributing", "security", "license"],
    ),
];
pub fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&#x27;")
}
pub fn slug(text: &str) -> String {
    let mut output = String::new();
    let mut separator = false;
    for character in text.to_lowercase().chars() {
        if character.is_alphanumeric() || matches!(character, '_' | '-') {
            if separator && !output.is_empty() {
                output.push('-');
            }
            separator = false;
            output.push(character);
        } else {
            separator = true;
        }
    }
    output.trim_matches('-').to_owned()
}
fn resolve(path: &Path) -> Result<PathBuf> {
    let mut output = PathBuf::new();
    for part in path.components() {
        match part {
            Component::ParentDir => {
                output.pop();
            }
            Component::CurDir => {}
            part => output.push(part.as_os_str()),
        }
        if let Ok(meta) = fs::symlink_metadata(&output)
            && meta.file_type().is_symlink()
        {
            output = fs::canonicalize(&output)?;
        }
    }
    match fs::canonicalize(&output) {
        Ok(canonical) => Ok(canonical),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            // Normalize existing ancestors too, including Windows verbatim
            // prefixes, while keeping links to not-yet-created files usable.
            let parent = output.parent().context("Path has no existing ancestor")?;
            let name = output.file_name().context("Path has no filename")?;
            Ok(resolve(parent)?.join(name))
        }
        Err(error) => Err(error.into()),
    }
}
pub fn route(root: &Path, path: &Path) -> Option<String> {
    let canonical = fs::canonicalize(root).ok()?;
    let root = canonical.as_path();
    let path = resolve(path).ok()?;
    let name = path.file_name()?.to_str()?;
    let stem = path.file_stem()?.to_str()?;
    if path == root.join("LICENSE") {
        return Some("/license/".into());
    }
    if path.parent() == Some(root) {
        return match name {
            "CONTRIBUTING.md" | "SECURITY.md" => Some(format!("/docs/{}/", stem.to_lowercase())),
            "README.zh-CN.md" => Some("/docs/overview-zh/".into()),
            "README.md" => Some("/docs/overview/".into()),
            _ => None,
        };
    }
    if path.parent() == Some(root.join("docs").as_path()) && !EXCLUDED.contains(&name) {
        return Some(if stem == "index" { "/docs/".into() } else { format!("/docs/{stem}/") });
    }
    None
}
fn quote(value: &str) -> String {
    value
        .as_bytes()
        .iter()
        .map(|&byte| {
            if byte.is_ascii_alphanumeric() || b"_.~-/".contains(&byte) {
                (byte as char).to_string()
            } else {
                format!("%{byte:02X}")
            }
        })
        .collect()
}
pub fn href(root: &Path, target: &str, source: &Path) -> String {
    let Ok(canonical) = fs::canonicalize(root) else { return "#".into() };
    let root = canonical.as_path();
    if let Some(capture) = SCHEME.captures(target) {
        return if matches!(&capture[1].to_lowercase()[..], "http" | "https" | "mailto") {
            target.into()
        } else {
            "#".into()
        };
    }
    if target.starts_with("//") {
        return "#".into();
    }
    let (target, fragment) = target.split_once('#').map_or((target, ""), |(a, b)| (a, b));
    let pathpart = target.split('?').next().unwrap();
    if pathpart.is_empty() {
        return if fragment.is_empty() { target.to_owned() } else { format!("{target}#{fragment}") };
    }
    let Ok(path) = resolve(&source.parent().unwrap().join(pathpart)) else { return "#".into() };
    if !path.starts_with(root) {
        return "#".into();
    }
    let fragment = if fragment.is_empty() { String::new() } else { format!("#{fragment}") };
    if path.is_file()
        && let Some(route) = route(root, &path)
    {
        return route + &fragment;
    }
    format!(
        "{REPOSITORY}/blob/main/{}{}",
        quote(&path.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/")),
        fragment
    )
}
pub fn inline(root: &Path, value: &str, source: &Path) -> String {
    let mut result = String::new();
    let mut end = 0;
    for part in INLINE.find_iter(value) {
        result.push_str(&escape(&value[end..part.start()]));
        end = part.end();
        let part = part.as_str();
        if part.starts_with('`') {
            result.push_str(&format!("<code>{}</code>", escape(&part[1..part.len() - 1])));
        } else if part.starts_with("**") {
            result.push_str(&format!("<strong>{}</strong>", escape(&part[2..part.len() - 2])));
        } else {
            let c = LINK.captures(part).unwrap();
            if part.starts_with('!') {
                result.push_str(&escape(&c[1]));
            } else {
                result.push_str(&format!("<a href=\"{}\">{}</a>", escape(&href(root, &c[2], source)), escape(&c[1])));
            }
        }
    }
    result.push_str(&escape(&value[end..]));
    result
}
pub type Section = (usize, String, String);
pub fn markdown(root: &Path, value: &str, source: &Path) -> (String, Vec<Section>) {
    render_markdown(root, value, source, &[])
}

fn render_markdown(root: &Path, value: &str, source: &Path, original: &[Section]) -> (String, Vec<Section>) {
    let lines = value.lines().collect::<Vec<_>>();
    let mut output = vec![];
    let mut sections = vec![];
    let mut identifiers = BTreeMap::<String, usize>::new();
    let mut index = 0;
    let heading = Regex::new(r"^(#{1,6})\s+(.+)$").unwrap();
    let item = Regex::new(r"^\s*(?:[-*]|\d+\.)\s+(.+)$").unwrap();
    let ordered = Regex::new(r"^\s*\d+\.").unwrap();
    let table_separator = Regex::new(r"^\|[\s:|\-]+\|?$").unwrap();
    let paragraph_stop = Regex::new(r"^(#{1,6}\s|```|\||>|\s*[-*]\s|\s*\d+\.\s)").unwrap();
    while index < lines.len() {
        let line = lines[index];
        if line.trim().is_empty() {
            index += 1;
            continue;
        }
        if let Some(language) = line.strip_prefix("```") {
            let mut code = vec![];
            index += 1;
            while index < lines.len() && !lines[index].starts_with("```") {
                code.push(lines[index]);
                index += 1;
            }
            let language = if language.trim().is_empty() { "text" } else { language.trim() };
            output.push(format!(
                "<div class=\"code-block\"><span>{}</span><pre><code>{}</code></pre></div>",
                escape(language),
                escape(&code.join("\n"))
            ));
            index += 1;
            continue;
        }
        if let Some(c) = heading.captures(line) {
            let level = c[1].len();
            let title = &c[2];
            let identifier = slug(title);
            let count = identifiers.entry(identifier.clone()).or_default();
            *count += 1;
            let identifier =
                if (2..=3).contains(&level) { original.get(sections.len()).map(|(_, _, id)| id.clone()) } else { None }
                    .unwrap_or_else(|| if *count > 1 { format!("{identifier}-{count}") } else { identifier });
            output.push(format!(
                "<h{level} id=\"{}\">{}<a class=\"anchor\" href=\"#{}\" aria-label=\"{}\">#</a></h{level}>",
                escape(&identifier),
                inline(root, title, source),
                escape(&identifier),
                escape(title)
            ));
            if (2..=3).contains(&level) {
                sections.push((level, title.to_owned(), identifier));
            }
            index += 1;
            continue;
        }
        if line.starts_with('|') && index + 1 < lines.len() && table_separator.is_match(lines[index + 1]) {
            let cells = |line: &str, tag: &str| {
                line.trim()
                    .trim_matches('|')
                    .split('|')
                    .map(|cell| format!("<{tag}>{}</{tag}>", inline(root, cell.trim(), source)))
                    .collect::<String>()
            };
            let mut table =
                format!("<div class=\"table-scroll\"><table><thead><tr>{}</tr></thead><tbody>", cells(line, "th"));
            index += 2;
            while index < lines.len() && lines[index].starts_with('|') {
                table.push_str(&format!("<tr>{}</tr>", cells(lines[index], "td")));
                index += 1;
            }
            table.push_str("</tbody></table></div>");
            output.push(table);
            continue;
        }
        if item.is_match(line) {
            let tag = if ordered.is_match(line) { "ol" } else { "ul" };
            let mut list = format!("<{tag}>");
            while index < lines.len() {
                let Some(c) = item.captures(lines[index]) else { break };
                list.push_str(&format!("<li>{}</li>", inline(root, &c[1], source)));
                index += 1;
            }
            output.push(list + &format!("</{tag}>"));
            continue;
        }
        if matches!(line.trim(), "---" | "***") {
            output.push("<hr>".into());
            index += 1;
            continue;
        }
        if line.starts_with('>') {
            let mut quote = String::new();
            for line in lines[index..].iter().map_while(|line| line.strip_prefix('>')) {
                if !quote.is_empty() {
                    quote.push(' ');
                }
                quote.push_str(line.trim_start());
                index += 1;
            }
            output.push(format!("<blockquote><p>{}</p></blockquote>", inline(root, &quote, source)));
            continue;
        }
        let mut paragraph = vec![];
        while index < lines.len() && !lines[index].trim().is_empty() && !paragraph_stop.is_match(lines[index]) {
            paragraph.push(lines[index]);
            index += 1;
        }
        if paragraph.is_empty() {
            paragraph.push(lines[index]);
            index += 1;
        }
        output.push(format!("<p>{}</p>", inline(root, &paragraph.join(" "), source)));
    }
    (output.join("\n"), sections)
}
pub fn copy_downloads(root: &Path, destination: &Path) -> Result<()> {
    let directory = root.join("web/downloads");
    let metadata = directory.join("cors-helper.json");
    ensure!(
        !fs::symlink_metadata(&metadata)?.file_type().is_symlink(),
        "Download metadata must not be a symbolic link"
    );
    let manifest = read_json(&metadata)?;
    let version = text(&manifest, "version")?;
    ensure!(Regex::new(r"^\d+\.\d+\.\d+$")?.is_match(version), "Invalid extension version");
    let packages = manifest["packages"].as_array().context("Invalid extension package list")?;
    let browsers = packages.iter().map(|item| text(item, "browser")).collect::<Result<BTreeSet<_>>>()?;
    ensure!(
        packages.len() == 3 && browsers == BTreeSet::from(["chromium", "firefox", "safari"]),
        "Invalid extension package list"
    );
    fs::create_dir_all(destination)?;
    for item in packages {
        let name = format!("harness-cors-helper-{}-{version}.zip", text(item, "browser")?);
        ensure!(text(item, "file")? == name, "Invalid extension package filename");
        let source = directory.join(&name);
        let meta = fs::symlink_metadata(&source)?;
        ensure!(meta.is_file() && !meta.file_type().is_symlink(), "Invalid extension package: {name}");
        let bytes = fs::read(source)?;
        ensure!(
            item["bytes"].as_u64() == Some(bytes.len() as u64) && text(item, "sha256")? == hash(&bytes),
            "Extension package checksum mismatch: {name}"
        );
        write(&destination.join(name), bytes)?;
    }
    write(&destination.join("cors-helper.json"), fs::read(metadata)?)?;
    Ok(())
}
pub fn render_client_install(root: &Path) -> Result<String> {
    let release = read_json(&root.join("site/releases.json"))?;
    let version = release["version"].as_str();
    ensure!(
        release["version"].is_null() || version.is_some_and(|v| Regex::new(r"^\d+\.\d+\.\d+$").unwrap().is_match(v)),
        "Invalid native release version"
    );
    let platforms = [
        ("macos", "macOS", "aarch64-apple-darwin", "tar.gz"),
        ("windows", "Windows", "x86_64-pc-windows-msvc", "zip"),
        ("linux", "Linux", "x86_64-unknown-linux-gnu", "tar.gz"),
    ];
    let mut validated = BTreeMap::new();
    for client in release["clients"].as_array().context("Invalid native release clients")? {
        let platform = text(client, "platform")?;
        let (_, _, target, extension) =
            platforms.iter().find(|p| p.0 == platform).context("Invalid native release platform")?;
        ensure!(version.is_some() && !validated.contains_key(platform), "Invalid native release platform");
        let version = version.unwrap();
        let file = text(client, "file")?;
        let format = if platform == "macos" && file == format!("i-{version}-macos-arm64-developer-id.dmg") {
            "dmg"
        } else {
            ensure!(file == format!("i-{version}-{target}-min-size.{extension}"), "Invalid native release asset");
            extension
        };
        ensure!(
            text(client, "target")? == *target && SHA.is_match(text(client, "sha256")?),
            "Invalid native release asset"
        );
        validated.insert(platform, (file, format));
    }
    let mut panels = vec![];
    for (platform, name, target, extension) in platforms {
        let client = validated.get(platform).copied();
        let url = if let Some((file, _)) = client {
            format!("{REPOSITORY}/releases/download/v{}/{file}", version.unwrap())
        } else {
            format!("{REPOSITORY}/releases")
        };
        let extension = client.map_or(extension, |(_, format)| format);
        let prefix = if extension == "dmg" { "install.macos.dmg" } else { "install.native" };
        let note =
            if extension == "dmg" { "install.macos.dmg.note".into() } else { format!("install.{platform}.note") };
        let (detail, step, step_detail) = if extension == "dmg" {
            (
                "无需 Rust、Node，挂载后可直接运行，无需先安装。",
                "挂载 DMG 并运行",
                "验证校验和，然后双击 DMG。在终端切换到可写的项目目录，再运行下面的命令。",
            )
        } else {
            (
                "无需 Rust、Node，解压后直接运行，不必先安装。",
                "解压，直接运行",
                "校验并解压，在解压目录的终端运行。工作区默认为当前终端目录。",
            )
        };
        let metadata = if client.is_some() { format!("v{} / {target}", version.unwrap()) } else { target.into() };
        let checksum = if client.is_some() {
            format!("<a class=\"checksum-link\" href=\"{url}.sha256\">SHA-256 ↓</a>")
        } else {
            String::new()
        };
        let (key, download) = if client.is_some() {
            ("install.download", "下载安装包 ↓")
        } else {
            ("install.pending", "查看发布进度 ↗")
        };
        let launch = match (platform, extension) {
            ("macos", "dmg") => format!("\"/Volumes/i {}/bin/i\"", version.unwrap()),
            ("windows", _) => ".\\bin\\i.exe".into(),
            _ => "./bin/i".into(),
        };
        let command = format!("{launch} install --add-to-path");
        let mut panel = include_str!("client-panel.html").trim_end().to_owned();
        for (key, value) in [
            ("PLATFORM", platform),
            ("NAME", name),
            ("EXTENSION", &extension.to_uppercase()),
            ("METADATA", &metadata),
            ("URL", &url),
            ("KEY", key),
            ("DOWNLOAD", download),
            ("CHECKSUM", &checksum),
            ("NOTE", &note),
            ("DETAIL_KEY", &format!("{prefix}.detail")),
            ("DETAIL", detail),
            ("STEP1_KEY", &format!("{prefix}.step1")),
            ("STEP1", step),
            ("STEP1_DETAIL_KEY", &format!("{prefix}.step1.detail")),
            ("STEP1_DETAIL", step_detail),
            ("LAUNCH", &launch),
            ("COMMAND", &command),
        ] {
            panel = panel.replace(&format!("{{{{{key}}}}}"), value);
        }
        panels.push(panel);
    }
    Ok(fs::read_to_string(root.join("site/install.html"))?.replace("{{NATIVE_CLIENTS}}", &panels.join("\n")))
}
struct Page {
    source: PathBuf,
    title: String,
    url: String,
    text: String,
    translations: BTreeMap<String, String>,
}
impl Page {
    fn language(&self) -> &'static str {
        if ENGLISH.contains(&self.source.file_name().unwrap().to_str().unwrap()) { "en" } else { "zh-CN" }
    }

    fn label_key(&self) -> String {
        let slug = self.url.strip_prefix("/docs/").unwrap_or(&self.url).trim_matches('/');
        format!("docs.page.{}", if slug.is_empty() { "index" } else { slug })
    }
}

fn section_links(sections: &[Section], lang: &str) -> String {
    sections
        .iter()
        .map(|(level, title, id)| {
            format!(
                "<a class=\"level-{level}\" href=\"#{}\" lang=\"{lang}\" dir=\"auto\">{}</a>",
                escape(id),
                escape(title)
            )
        })
        .collect()
}

fn docs_label(messages: &Value, key: &str) -> Result<String> {
    Ok(format!("<span data-site-i18n=\"{key}\">{}</span>", escape(text(messages, key)?)))
}

fn docs_link(page: &Page, messages: &Value) -> String {
    let key = page.label_key();
    if let Some(label) = messages[&key].as_str() {
        format!("<a href=\"{}\" data-site-i18n=\"{key}\">{}</a>", page.url, escape(label))
    } else {
        format!("<a href=\"{}\" lang=\"{}\" dir=\"auto\">{}</a>", page.url, page.language(), escape(&page.title))
    }
}

fn docs_index(pages: &[Page], messages: &Value) -> Result<(String, String)> {
    let mut body = format!(
        "<article><h1 data-docs-title>{}</h1><p>{}</p>",
        docs_label(messages, "docs.page.index")?,
        docs_label(messages, "docs.intro")?
    );
    let mut toc = String::new();
    for (group, id, slugs) in DOC_GROUPS {
        let key = format!("docs.group.{group}");
        let label = docs_label(messages, &key)?;
        body.push_str(&format!(
            "<h2 id=\"{id}\">{label}</h2><p>{}</p><ul>",
            docs_label(messages, &format!("{key}.detail"))?
        ));
        toc.push_str(&format!("<a class=\"level-2\" href=\"#{id}\">{label}</a>"));
        for slug in *slugs {
            let url = if *slug == "license" { "/license/".into() } else { format!("/docs/{slug}/") };
            let page = pages.iter().find(|page| page.url == url).with_context(|| format!("Missing doc: {url}"))?;
            text(messages, &page.label_key())?;
            body.push_str(&format!("<li>{}</li>", docs_link(page, messages)));
        }
        body.push_str("</ul>");
    }
    body.push_str("</article>");
    Ok((body, toc))
}
struct Renderer<'a> {
    staging: &'a Path,
    origin: &'a str,
    template: String,
    sidebar: String,
    languages: String,
    messages: Value,
}
impl Renderer<'_> {
    fn emit(&self, url: &str, title: &str, body: &str, toc: &str, docs: bool) -> Result<()> {
        let home = url == "/";
        let plugins = url == "/plugins/";
        let assets = if home {
            "<link rel=\"stylesheet\" href=\"/assets/home.css\">"
        } else if plugins {
            "<link rel=\"stylesheet\" href=\"/assets/plugins.css\"><script type=\"module\" src=\"/assets/plugins.js\"></script>"
        } else if docs {
            "<link rel=\"stylesheet\" href=\"/assets/docs.css\">"
        } else {
            ""
        };
        let mut rendered = if plugins {
            self.template.replace("connect-src 'self';", "connect-src 'self' https://qinains.github.io;")
        } else {
            self.template.clone()
        };
        for (key, value) in [
            ("TITLE", escape(title)),
            ("CANONICAL", escape(&format!("{}{url}", self.origin))),
            ("BODY", body.into()),
            ("SIDEBAR", if docs { self.sidebar.clone() } else { String::new() }),
            ("TOC", toc.into()),
            ("LAYOUT", if docs { "documentation" } else { "landing" }.into()),
            (
                "PAGE_KIND",
                if home {
                    "home"
                } else if plugins {
                    "plugins"
                } else {
                    "docs"
                }
                .into(),
            ),
            ("PAGE_ASSETS", assets.into()),
        ] {
            rendered = rendered.replace(&format!("{{{{{key}}}}}"), &value);
        }
        rendered = Regex::new(r#"(?s)(<select\b[^>]*id="site-language"[^>]*>).*?(</select>)"#)?
            .replace_all(&rendered, |c: &Captures| format!("{}{}{}", &c[1], self.languages, &c[2]))
            .into_owned();
        let pattern = Regex::new(r#"(<[^>]+\bdata-site-i18n="([^"]+)"[^>]*>)[^<]*(</[^>]+>)"#)?;
        for c in pattern.captures_iter(&rendered) {
            ensure!(self.messages[&c[2]].is_string(), "unknown site translation: {}", &c[2]);
        }
        rendered = pattern
            .replace_all(&rendered, |c: &Captures| {
                format!("{}{}{}", &c[1], escape(self.messages[&c[2]].as_str().unwrap()), &c[3])
            })
            .into_owned();
        write(&self.staging.join(url.trim_start_matches('/')).join("index.html"), rendered)
    }
}
pub fn build(root: &Path, origin: &str, output: &Path) -> Result<usize> {
    crate::translations::generate(root, true)?;
    let origin = origin.trim_end_matches('/');
    let parsed = url::Url::parse(origin)?;
    ensure!(
        parsed.scheme() == "https"
            && parsed.host_str().is_some()
            && parsed.username().is_empty()
            && parsed.password().is_none()
            && parsed.query().is_none()
            && parsed.fragment().is_none()
            && parsed.path() == "/",
        "Public origin must be an HTTPS origin"
    );
    ensure!(
        !output.exists() || output.join(".harness-site-build").is_file(),
        "Refusing to replace an unrecognized output directory"
    );
    let root = fs::canonicalize(root)?;
    let registry = read_json(&root.join("locales/languages.json"))?;
    let registry = registry.as_array().context("languages array")?;
    let mut documents = fs::read_dir(root.join("docs"))?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| {
            path.extension().is_some_and(|e| e == "md")
                && !EXCLUDED.contains(&path.file_name().unwrap().to_str().unwrap())
        })
        .collect::<Vec<_>>();
    documents.sort();
    documents.extend(
        ["README.md", "README.zh-CN.md", "CONTRIBUTING.md", "SECURITY.md", "LICENSE"].map(|name| root.join(name)),
    );
    let mut pages = vec![];
    for source in documents {
        let content = fs::read_to_string(&source)?;
        let title = content
            .lines()
            .find_map(|line| line.strip_prefix("# ").map(|s| s.trim_start_matches(['#', ' ']).to_owned()))
            .unwrap_or_else(|| source.file_stem().unwrap().to_string_lossy().into());
        let url = route(&root, &source).context("document has no public route")?;
        pages.push(Page { source, title, url, text: content, translations: BTreeMap::new() });
    }
    for locale in registry {
        let lang = text(locale, "code")?;
        let directory = root.join("docs/translations").join(lang);
        if !directory.is_dir() {
            continue;
        }
        for entry in fs::read_dir(directory)? {
            let path = entry?.path();
            if !path.is_file() || path.extension().is_none_or(|ext| ext != "md") {
                continue;
            }
            let page = pages
                .iter_mut()
                .find(|p| {
                    p.url != "/docs/"
                        && p.source.starts_with(root.join("docs"))
                        && p.source.file_name() == path.file_name()
                })
                .with_context(|| format!("No public source article for {}", path.display()))?;
            ensure!(page.language() != lang, "Duplicate source language in {}", path.display());
            page.translations.insert(lang.to_owned(), fs::read_to_string(&path)?);
        }
    }
    let featured = FEATURED
        .iter()
        .filter_map(|slug| {
            pages.iter().position(|page| page.source.file_stem().unwrap().to_string_lossy().to_lowercase() == *slug)
        })
        .collect::<Vec<_>>();
    let messages = read_json(&root.join("locales/site/zh-CN.json"))?;
    let link = |page: &Page| docs_link(page, &messages);
    let sidebar = format!(
        "<details class=\"docs-menu\" open><summary data-site-i18n=\"docs.navigation\">文档目录</summary><nav aria-label=\"文档\" data-site-i18n-aria=\"nav.docs\">{}<details><summary data-site-i18n=\"docs.all\">全部参考文档</summary>{}</details></nav></details>",
        featured.iter().map(|&i| link(&pages[i])).collect::<String>(),
        pages.iter().enumerate().filter(|(i, _)| !featured.contains(i)).map(|(_, p)| link(p)).collect::<String>()
    );
    let languages = registry
        .iter()
        .map(|item| {
            Ok(format!(
                "<option value=\"{}\" lang=\"{}\" dir=\"{}\">{}</option>",
                text(item, "code")?,
                text(item, "code")?,
                text(item, "dir")?,
                escape(text(item, "name")?)
            ))
        })
        .collect::<Result<String>>()?;
    let parent = output.parent().context("output parent")?;
    fs::create_dir_all(parent)?;
    let staging = tempfile::Builder::new().prefix("site-").tempdir_in(parent)?;
    let renderer = Renderer {
        staging: staging.path(),
        origin,
        template: fs::read_to_string(root.join("site/template.html"))?,
        sidebar,
        languages,
        messages,
    };
    renderer.emit(
        "/",
        text(&renderer.messages, "page.title")?,
        &fs::read_to_string(root.join("site/home.html"))?.replace("{{CLIENT_INSTALL}}", &render_client_install(&root)?),
        "",
        false,
    )?;
    renderer.emit(
        "/plugins/",
        &format!("{} · i", text(&renderer.messages, "plugins.title")?),
        &fs::read_to_string(root.join("site/plugins.html"))?,
        "",
        false,
    )?;
    for page in &pages {
        let (body, links) = if page.url == "/docs/" {
            docs_index(&pages, &renderer.messages)?
        } else {
            let (body, sections) = markdown(&root, &page.text, &page.source);
            let lang = page.language();
            let links = section_links(&sections, lang);
            let hidden = if lang == "zh-CN" { " hidden" } else { "" };
            let label = page.label_key();
            let label = if renderer.messages[&label].is_string() { label.as_str() } else { "" };
            let edit = escape(&page.source.strip_prefix(&root)?.to_string_lossy());
            let mut templates = String::new();
            if !page.translations.is_empty() {
                let variant = |code: &str, content: &str, toc: &str, edit: &str| -> Result<String> {
                    let dir =
                        registry.iter().find(|locale| locale["code"] == code).context("Unknown article language")?;
                    let dir = text(dir, "dir")?;
                    Ok(format!(
                        "<template data-docs-variant=\"{code}\" data-docs-dir=\"{dir}\" data-docs-edit=\"{REPOSITORY}/edit/main/{edit}\"><article>{content}</article><nav>{toc}</nav></template>"
                    ))
                };
                templates.push_str(&variant(lang, &body, &links, &edit)?);
                for (code, content) in &page.translations {
                    let (content, translated) = render_markdown(&root, content, &page.source, &sections);
                    ensure!(
                        translated.iter().map(|s| s.0).eq(sections.iter().map(|s| s.0)),
                        "Section structure differs in {code} translation of {edit}"
                    );
                    let toc = section_links(&translated, code);
                    let edit =
                        format!("docs/translations/{code}/{}", page.source.file_name().unwrap().to_string_lossy());
                    templates.push_str(&variant(code, &content, &toc, &escape(&edit))?);
                }
            }
            let body = format!(
                "<p class=\"docs-language-notice\" data-docs-source=\"{lang}\" data-docs-label=\"{label}\"{hidden}><span data-site-i18n=\"docs.original\">暂无所选语言版本，下方显示原文。</span> <bdi data-docs-source-label></bdi></p><article lang=\"{lang}\" dir=\"ltr\">{body}</article><p class=\"source-link\"><a href=\"{REPOSITORY}/edit/main/{}\" data-site-i18n=\"docs.edit\">在 GitHub 改进本页</a></p>",
                edit
            );
            (body + &templates, links)
        };
        let toc = format!(
            "<nav aria-label=\"本页内容\" data-site-i18n-aria=\"docs.toc\"><strong data-site-i18n=\"docs.toc\">本页内容</strong>{links}</nav>"
        );
        renderer.emit(&page.url, &format!("{} · i", page.title), &body, &toc, true)?;
    }
    copy_tree(&root.join("site/assets"), &staging.path().join("assets"))?;
    write(&staging.path().join("third-party-licenses.txt"), fs::read(root.join("THIRD_PARTY_LICENSES.txt"))?)?;
    write(
        &staging.path().join("search-index.json"),
        serde_json::to_string(
            &pages
                .iter()
                .map(|p| {
                    let key = p.label_key();
                    json!({"title":p.title,"url":p.url,"text":p.text,"lang":p.language(),
                    "label":renderer.messages[&key].as_str().map(|_| key.as_str()), "translations":p.translations})
                })
                .collect::<Vec<_>>(),
        )?,
    )?;
    let urls = ["/", "/plugins/"]
        .into_iter()
        .chain(pages.iter().map(|p| p.url.as_str()))
        .chain(["/app/"])
        .map(|url| format!("<url><loc>{}</loc></url>", escape(&format!("{origin}{url}"))))
        .collect::<String>();
    write(
        &staging.path().join("sitemap.xml"),
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?><urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">{urls}</urlset>"
        ),
    )?;
    write(&staging.path().join("robots.txt"), format!("User-agent: *\nAllow: /\nSitemap: {origin}/sitemap.xml\n"))?;
    write(&staging.path().join("CNAME"), format!("{}\n", parsed.host_str().unwrap()))?;
    write(&staging.path().join(".nojekyll"), "")?;
    let manifest = read_json(&root.join("web/assets.json"))?;
    let expected = manifest["precache"]
        .as_array()
        .context("precache array")?
        .iter()
        .map(|item| Ok((text(item, "url")?.to_owned(), text(item, "sha256")?.to_owned())))
        .collect::<Result<BTreeMap<_, _>>>()?;
    let assets = manifest["assets"]
        .as_array()
        .context("assets array")?
        .iter()
        .map(|v| v.as_str().context("asset string"))
        .chain([Ok("assets.json")]);
    let web = fs::canonicalize(root.join("web"))?;
    for name in assets {
        let name = name?;
        ensure!(
            !Path::new(name).is_absolute()
                && Path::new(name).components().all(|part| matches!(part, Component::Normal(_))),
            "Invalid public asset: {name}"
        );
        let source = fs::canonicalize(web.join(name)).with_context(|| format!("Invalid public asset: {name}"))?;
        ensure!(source.starts_with(&web) && source.is_file(), "Invalid public asset: {name}");
        let bytes = fs::read(source)?;
        if let Some(expected) = expected.get(name) {
            ensure!(&hash(&bytes) == expected, "Stale application build; rebuild before publishing: {name}");
        }
        write(&staging.path().join("app").join(name), bytes)?;
    }
    copy_downloads(&root, &staging.path().join("downloads"))?;
    write(&staging.path().join(".harness-site-build"), "generated\n")?;
    drop(renderer);
    if output.exists() {
        fs::remove_dir_all(output)?;
    }
    fs::rename(staging.path(), output)?;
    Ok(pages.len())
}
