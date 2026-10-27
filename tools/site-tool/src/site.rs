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
    let lines = value.lines().collect::<Vec<_>>();
    let mut output = vec![];
    let mut sections = vec![];
    let mut identifiers = BTreeMap::<String, usize>::new();
    let mut index = 0;
    let heading = Regex::new(r"^(#{1,6})\s+(.+)$").unwrap();
    let item = Regex::new(r"^\s*(?:[-*]|\d+\.)\s+(.+)$").unwrap();
    let ordered = Regex::new(r"^\s*\d+\.").unwrap();
    let table_separator = Regex::new(r"^\|[\s:|\-]+\|?$").unwrap();
    let paragraph_stop = Regex::new(r"^(#{1,6}\s|```|\||\s*[-*]\s|\s*\d+\.\s)").unwrap();
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
            let identifier = if *count > 1 { format!("{identifier}-{count}") } else { identifier };
            output.push(format!(
                "<h{level} id=\"{}\">{}<a class=\"anchor\" href=\"#{}\" aria-label=\"链接到此节\">#</a></h{level}>",
                escape(&identifier),
                inline(root, title, source),
                escape(&identifier)
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
        ensure!(
            text(client, "target")? == *target
                && text(client, "file")? == format!("i-{}-{target}-min-size.{extension}", version.unwrap())
                && SHA.is_match(text(client, "sha256")?),
            "Invalid native release asset"
        );
        validated.insert(platform, client);
    }
    let mut panels = vec![];
    for (platform, name, target, extension) in platforms {
        let client = validated.get(platform);
        let url = if let Some(client) = client {
            format!("{REPOSITORY}/releases/download/v{}/{}", version.unwrap(), text(client, "file")?)
        } else {
            format!("{REPOSITORY}/releases")
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
        let launch = if platform == "windows" { ".\\bin\\i.exe" } else { "./bin/i" };
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
            ("NOTE", &format!("install.{platform}.note")),
            ("LAUNCH", launch),
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
        pages.push(Page { source, title, url, text: content });
    }
    let featured = FEATURED
        .iter()
        .filter_map(|slug| {
            pages.iter().position(|page| page.source.file_stem().unwrap().to_string_lossy().to_lowercase() == *slug)
        })
        .collect::<Vec<_>>();
    let link = |page: &Page| format!("<a href=\"{}\">{}</a>", page.url, escape(&page.title));
    let sidebar = format!(
        "<details class=\"docs-menu\" open><summary>文档目录</summary><nav aria-label=\"文档\">{}<details><summary>全部参考文档</summary>{}</details></nav></details>",
        featured.iter().map(|&i| link(&pages[i])).collect::<String>(),
        pages.iter().enumerate().filter(|(i, _)| !featured.contains(i)).map(|(_, p)| link(p)).collect::<String>()
    );
    let registry = read_json(&root.join("locales/languages.json"))?;
    let languages = registry
        .as_array()
        .context("languages array")?
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
        messages: read_json(&root.join("locales/site/zh-CN.json"))?,
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
        let (body, sections) = markdown(&root, &page.text, &page.source);
        let toc = format!(
            "<nav aria-label=\"本页目录\"><strong>本页内容</strong>{}</nav>",
            sections
                .iter()
                .map(|(level, title, id)| format!(
                    "<a class=\"level-{level}\" href=\"#{}\">{}</a>",
                    escape(id),
                    escape(title)
                ))
                .collect::<String>()
        );
        let lang = if ENGLISH.contains(&page.source.file_name().unwrap().to_str().unwrap()) { "en" } else { "zh-CN" };
        renderer.emit(&page.url,&format!("{} · i 文档",page.title),&format!("<article lang=\"{lang}\">{body}<p class=\"source-link\" lang=\"zh-CN\"><a href=\"{REPOSITORY}/edit/main/{}\">在 GitHub 改进本页</a></p></article>",page.source.strip_prefix(&root)?.to_string_lossy()),&toc,true)?;
    }
    copy_tree(&root.join("site/assets"), &staging.path().join("assets"))?;
    write(&staging.path().join("third-party-licenses.txt"), fs::read(root.join("THIRD_PARTY_LICENSES.txt"))?)?;
    write(
        &staging.path().join("search-index.json"),
        serde_json::to_string(
            &pages.iter().map(|p| json!({"title":p.title,"url":p.url,"text":p.text})).collect::<Vec<_>>(),
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
