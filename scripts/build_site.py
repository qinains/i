#!/usr/bin/env python3
"""Build the public static site and copy only verified application assets."""
import argparse
import hashlib
import html
import json
from pathlib import Path
import re
import shutil
import tempfile
import subprocess
import sys
from urllib.parse import quote, urlsplit

ROOT = Path(__file__).resolve().parent.parent
REPOSITORY = "https://github.com/qinains/i"
EXCLUDED = {"tools-composer-verification.md", "dependency-size-history.md", "dependency-audit.md", "dependency-size.md", "size-report.md"}
FEATURED = ["index", "connect-model", "system-overview", "plugin-development", "plugin-publishing", "wasm-plugin-reference", "plugins", "wasm", "deployment", "providers", "file-tools", "cors-extension", "site-deployment", "opfs-concurrency"]
ENGLISH_DOCUMENTS = {"README.md", "CONTRIBUTING.md", "SECURITY.md", "LICENSE", "compaction.md", "console.md", "composer.md", "file-tools.md", "opfs-concurrency.md", "network.md", "wasm-size.md"}


def slug(text):
    return re.sub(r"[^\w\-\u4e00-\u9fff]+", "-", text.lower()).strip("-")


def route(path):
    path = path.resolve()
    if path == ROOT / "LICENSE":
        return "/license/"
    if path in [ROOT / "CONTRIBUTING.md", ROOT / "SECURITY.md"]:
        return "/docs/" + path.stem.lower() + "/"
    if path.name == "README.zh-CN.md" and path.parent == ROOT:
        return "/docs/overview-zh/"
    if path.name == "README.md" and path.parent == ROOT:
        return "/docs/overview/"
    if path.parent == ROOT / "docs" and path.name not in EXCLUDED:
        return "/docs/" + ("" if path.stem == "index" else path.stem + "/")
    return None


def href(target, source):
    parsed = urlsplit(target)
    if parsed.scheme:
        return target if parsed.scheme in ["https", "http", "mailto"] else "#"
    if target.startswith("//"):
        return "#"
    if not parsed.path:
        return target
    path = (source.parent / parsed.path).resolve()
    if not path.is_relative_to(ROOT):
        return "#"
    public = route(path) if path.is_file() else None
    if public:
        return public + ("#" + parsed.fragment if parsed.fragment else "")
    return REPOSITORY + "/blob/main/" + quote(str(path.relative_to(ROOT))) + ("#" + parsed.fragment if parsed.fragment else "")


def inline(text, source):
    parts = re.split(r"(`[^`]+`|!\[[^\]]*\]\([^)]+\)|\[[^\]]+\]\([^)]+\)|\*\*[^*]+\*\*)", text)
    result = []
    for part in parts:
        if part.startswith("`") and part.endswith("`"):
            result.append("<code>" + html.escape(part[1:-1]) + "</code>")
        elif part.startswith("!["):
            match = re.fullmatch(r"!\[([^\]]*)\]\(([^)]+)\)", part)
            result.append(html.escape(match[1]) if match else html.escape(part))
        elif part.startswith("["):
            match = re.fullmatch(r"\[([^\]]+)\]\(([^)]+)\)", part)
            result.append(f'<a href="{html.escape(href(match[2], source), quote=True)}">{html.escape(match[1])}</a>' if match else html.escape(part))
        elif part.startswith("**") and part.endswith("**"):
            result.append("<strong>" + html.escape(part[2:-2]) + "</strong>")
        else:
            result.append(html.escape(part))
    return "".join(result)


def markdown(text, source):
    lines = text.splitlines(); result = []; toc = []; index = 0; identifiers = {}
    while index < len(lines):
        line = lines[index]
        if not line.strip():
            index += 1; continue
        if line.startswith("```"):
            language = line[3:].strip(); code = []; index += 1
            while index < len(lines) and not lines[index].startswith("```"):
                code.append(lines[index]); index += 1
            result.append(f'<div class="code-block"><span>{html.escape(language or "text")}</span><pre><code>{html.escape(chr(10).join(code))}</code></pre></div>'); index += 1; continue
        heading = re.match(r"^(#{1,6})\s+(.+)$", line)
        if heading:
            level, title = len(heading[1]), heading[2]; identifier = slug(title)
            identifiers[identifier] = identifiers.get(identifier, 0) + 1
            if identifiers[identifier] > 1:
                identifier += "-" + str(identifiers[identifier])
            result.append(f'<h{level} id="{html.escape(identifier)}">{inline(title, source)}<a class="anchor" href="#{html.escape(identifier)}" aria-label="链接到此节">#</a></h{level}>')
            if 2 <= level <= 3:
                toc.append((level, title, identifier))
            index += 1; continue
        if line.startswith("|") and index + 1 < len(lines) and re.match(r"^\|[\s:|\-]+\|?$", lines[index + 1]):
            cells = lambda value: [inline(cell.strip(), source) for cell in value.strip().strip("|").split("|")]
            table = '<div class="table-scroll"><table><thead><tr>' + "".join("<th>" + cell + "</th>" for cell in cells(line)) + "</tr></thead><tbody>"; index += 2
            while index < len(lines) and lines[index].startswith("|"):
                table += "<tr>" + "".join("<td>" + cell + "</td>" for cell in cells(lines[index])) + "</tr>"; index += 1
            result.append(table + "</tbody></table></div>"); continue
        item = re.match(r"^\s*(?:[-*]|\d+\.)\s+(.+)$", line)
        if item:
            ordered = bool(re.match(r"^\s*\d+\.", line)); tag = "ol" if ordered else "ul"; items = []
            while index < len(lines):
                item = re.match(r"^\s*(?:[-*]|\d+\.)\s+(.+)$", lines[index])
                if not item:
                    break
                items.append("<li>" + inline(item[1], source) + "</li>"); index += 1
            result.append(f"<{tag}>" + "".join(items) + f"</{tag}>"); continue
        if line.strip() in ["---", "***"]:
            result.append("<hr>"); index += 1; continue
        paragraph = []
        while index < len(lines) and lines[index].strip() and not re.match(r"^(#{1,6}\s|```|\||\s*[-*]\s|\s*\d+\.\s)", lines[index]):
            paragraph.append(lines[index]); index += 1
        if not paragraph:
            paragraph.append(lines[index]); index += 1
        result.append("<p>" + inline(" ".join(paragraph), source) + "</p>")
    return "\n".join(result), toc


def copy_downloads(destination):
    directory = ROOT / "web/downloads"
    metadata = directory / "cors-helper.json"
    if metadata.is_symlink():
        raise SystemExit("Download metadata must not be a symbolic link")
    manifest = json.loads(metadata.read_text())
    version = manifest["version"]
    if not re.fullmatch(r"\d+\.\d+\.\d+", version):
        raise SystemExit("Invalid extension version")
    packages = manifest["packages"]
    if len(packages) != 3 or {item["browser"] for item in packages} != {"chromium", "firefox", "safari"}:
        raise SystemExit("Invalid extension package list")
    destination.mkdir(parents=True)
    for item in packages:
        name = f'harness-cors-helper-{item["browser"]}-{version}.zip'
        if item["file"] != name:
            raise SystemExit("Invalid extension package filename")
        source = directory / name
        if source.is_symlink() or not source.is_file():
            raise SystemExit("Invalid extension package: " + name)
        data = source.read_bytes()
        if len(data) != item["bytes"] or hashlib.sha256(data).hexdigest() != item["sha256"]:
            raise SystemExit("Extension package checksum mismatch: " + name)
        (destination / name).write_bytes(data)
    shutil.copyfile(metadata, destination / metadata.name)


def render_client_install():
    release = json.loads((ROOT / 'site/releases.json').read_text())
    version, clients = release['version'], release['clients']
    if version is not None and not re.fullmatch(r'\d+\.\d+\.\d+', version):
        raise SystemExit('Invalid native release version')
    platforms = {'macos': ('macOS', 'aarch64-apple-darwin', 'tar.gz'), 'windows': ('Windows', 'x86_64-pc-windows-msvc', 'zip'), 'linux': ('Linux', 'x86_64-unknown-linux-gnu', 'tar.gz')}
    validated = {}
    for client in clients:
        platform = client['platform']
        if not version or platform not in platforms or platform in validated:
            raise SystemExit('Invalid native release platform')
        name, target, extension = platforms[platform]
        if client['target'] != target or client['file'] != f'i-{version}-{target}-min-size.{extension}' or not re.fullmatch(r'[a-f0-9]{64}', client['sha256']):
            raise SystemExit('Invalid native release asset')
        validated[platform] = client
    panels = []
    for platform, (name, target, extension) in platforms.items():
        client = validated.get(platform)
        url = f'{REPOSITORY}/releases/download/v{version}/{client["file"]}' if client else REPOSITORY + '/releases'
        key, download = ('install.download', '下载安装包 ↓') if client else ('install.pending', '查看发布进度 ↗')
        metadata = f'v{version} / {target}' if client else target
        checksum = f'<a class="checksum-link" href="{url}.sha256">SHA-256 ↓</a>' if client else ''
        command = '.\\bin\\i.exe install --add-to-path' if platform == 'windows' else './bin/i install --add-to-path'
        launch = '.\\bin\\i.exe' if platform == 'windows' else './bin/i'
        note = 'install.macos.note' if platform == 'macos' else 'install.linux.note' if platform == 'linux' else 'install.windows.note'
        panels.append(f'''<div id="client-{platform}" class="client-panel" role="tabpanel" aria-labelledby="client-tab-{platform}" tabindex="0" hidden>
<div class="client-summary"><span class="client-kind">NATIVE CLI / {extension.upper()}</span><h3>{name}</h3><p class="client-target">{metadata}</p><p data-site-i18n="install.native.detail">无需 Rust、Node 或 Python，解压后直接运行，不必先安装。</p><a class="button client-download" href="{url}" data-site-i18n="{key}">{download}</a>{checksum}<p class="client-fineprint" data-site-i18n="{note}"></p></div>
<div><ol class="install-steps"><li><h4 data-site-i18n="install.native.step1">解压，直接运行</h4><p data-site-i18n="install.native.step1.detail">校验并解压，在解压目录的终端运行。工作区默认为当前终端目录。</p><pre><code>{launch}</code></pre></li><li><h4 data-site-i18n="install.native.step2">首次自动配置，然后开始</h4><p data-site-i18n="install.native.step2.detail">未配置时自动打开向导，填写 API URL、模型与密钥后进入会话。以后运行同一程序即可；无需另跑 setup 或 start。密钥隐藏输入。</p></li></ol><details data-native-install><summary data-site-i18n="install.native.step3">可选：安装后，在项目目录输入 i</summary><p data-site-i18n="install.native.step3.detail">仅想在任意目录输入 i 时才需要。安装到当前用户并更新 PATH；打开新终端、进入项目目录，输入 i。i 启动会话，不会自行安装。</p><pre><code>{command}</code></pre></details></div></div>''')
    return (ROOT / 'site/install.html').read_text().replace('{{NATIVE_CLIENTS}}', '\n'.join(panels))


def main():
    subprocess.run([sys.executable, str(ROOT / "scripts/build_translations.py"), "--check"], check=True)
    parser = argparse.ArgumentParser(); parser.add_argument("--origin", default="https://i.lninl.com"); args = parser.parse_args()
    origin = args.origin.rstrip("/")
    if not origin.startswith("https://"):
        raise SystemExit("Public origin must use HTTPS")
    documents = sorted(path for path in (ROOT / "docs").glob("*.md") if path.name not in EXCLUDED)
    documents += [ROOT / "README.md", ROOT / "README.zh-CN.md", ROOT / "CONTRIBUTING.md", ROOT / "SECURITY.md", ROOT / "LICENSE"]
    pages = []
    for path in documents:
        text = path.read_text(); title = next((line.lstrip("# ") for line in text.splitlines() if line.startswith("# ")), path.stem)
        pages.append({"source": path, "title": title, "url": route(path), "text": text})
    by_slug = {page["source"].stem.lower(): page for page in pages}
    featured = [by_slug[key] for key in FEATURED if key in by_slug]
    sidebar = '<details class="docs-menu" open><summary>文档目录</summary><nav aria-label="文档">' + "".join(f'<a href="{page["url"]}">{html.escape(page["title"])}</a>' for page in featured) + '<details><summary>全部参考文档</summary>' + "".join(f'<a href="{page["url"]}">{html.escape(page["title"])}</a>' for page in pages if page not in featured) + "</details></nav></details>"
    template = (ROOT / "site/template.html").read_text()
    default_messages = json.loads((ROOT / "locales/site/zh-CN.json").read_text())
    language_options = "".join(f'<option value="{item["code"]}" lang="{item["code"]}" dir="{item["dir"]}">{html.escape(item["name"])}</option>' for item in json.loads((ROOT / "locales/languages.json").read_text()))
    staging = Path(tempfile.mkdtemp(prefix="site-", dir=ROOT / "dist")); output = ROOT / "dist/site"
    def emit(url, title, body, toc="", docs=False):
        canonical = origin + url; destination = staging / url.lstrip("/") / "index.html"; destination.parent.mkdir(parents=True, exist_ok=True)
        home = url == "/"
        plugins = url == "/plugins/"
        values = {"TITLE": html.escape(title), "CANONICAL": html.escape(canonical, quote=True), "BODY": body, "SIDEBAR": sidebar if docs else "", "TOC": toc, "LAYOUT": "documentation" if docs else "landing", "PAGE_KIND": "home" if home else "docs", "PAGE_ASSETS": '<link rel="stylesheet" href="/assets/home.css">' if home else ""}
        rendered = template
        if plugins:
            values["PAGE_KIND"] = "plugins"
            values["PAGE_ASSETS"] = '<link rel="stylesheet" href="/assets/plugins.css"><script type="module" src="/assets/plugins.js"></script>'
            rendered = rendered.replace("connect-src 'self';", "connect-src 'self' https://qinains.github.io;")
        for key, value in values.items():
            rendered = rendered.replace("{{" + key + "}}", value)
        rendered = re.sub(r'(<select\b[^>]*id="site-language"[^>]*>).*?(</select>)', lambda match: match[1] + language_options + match[2], rendered, flags=re.S)
        rendered = re.sub(r'(<[^>]+\bdata-site-i18n="([^"]+)"[^>]*>)[^<]*(</[^>]+>)', lambda match: match[1] + html.escape(default_messages[match[2]]) + match[3], rendered)
        destination.write_text(rendered)
    emit("/", default_messages["page.title"], (ROOT / "site/home.html").read_text().replace('{{CLIENT_INSTALL}}', render_client_install()))
    emit("/plugins/", default_messages["plugins.title"] + " · i", (ROOT / "site/plugins.html").read_text())
    for page in pages:
        body, sections = markdown(page["text"], page["source"])
        toc = '<nav aria-label="本页目录"><strong>本页内容</strong>' + "".join(f'<a class="level-{level}" href="#{html.escape(identifier)}">{html.escape(title)}</a>' for level, title, identifier in sections) + "</nav>"
        language = "en" if page["source"].name in ENGLISH_DOCUMENTS else "zh-CN"
        emit(page["url"], page["title"] + " · i 文档", '<article lang="' + language + '">' + body + '<p class="source-link" lang="zh-CN"><a href="' + REPOSITORY + '/edit/main/' + str(page["source"].relative_to(ROOT)) + '">在 GitHub 改进本页</a></p></article>', toc, True)
    shutil.copytree(ROOT / "site/assets", staging / "assets")
    shutil.copyfile(ROOT / "THIRD_PARTY_LICENSES.txt", staging / "third-party-licenses.txt")
    (staging / "search-index.json").write_text(json.dumps([{key: page[key] for key in ["title", "url", "text"]} for page in pages], ensure_ascii=False))
    urls = ["/", "/plugins/"] + [page["url"] for page in pages] + ["/app/"]
    (staging / "sitemap.xml").write_text('<?xml version="1.0" encoding="UTF-8"?><urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">' + "".join("<url><loc>" + html.escape(origin + url) + "</loc></url>" for url in urls) + "</urlset>")
    (staging / "robots.txt").write_text("User-agent: *\nAllow: /\nSitemap: " + origin + "/sitemap.xml\n")
    (staging / "CNAME").write_text(urlsplit(origin).hostname + "\n"); (staging / ".nojekyll").write_text("")
    manifest = json.loads((ROOT / "web/assets.json").read_text())
    expected = {item["url"]: item["sha256"] for item in manifest["precache"]}
    for name in manifest["assets"] + ["assets.json"]:
        source = (ROOT / "web" / name).resolve()
        if not source.is_relative_to(ROOT / "web") or not source.is_file():
            raise SystemExit("Invalid public asset: " + name)
        if name in expected and hashlib.sha256(source.read_bytes()).hexdigest() != expected[name]:
            raise SystemExit("Stale application build; rebuild before publishing: " + name)
        target = staging / "app" / name; target.parent.mkdir(parents=True, exist_ok=True); shutil.copyfile(source, target)
    copy_downloads(staging / "downloads")
    (staging / ".harness-site-build").write_text("generated\n")
    if output.exists():
        if not (output / ".harness-site-build").is_file():
            raise SystemExit("Refusing to replace an unrecognized output directory")
        shutil.rmtree(output)
    staging.rename(output)
    print(f"Built {len(pages)} documentation pages and verified /app/ assets into {output}")


if __name__ == "__main__":
    main()
