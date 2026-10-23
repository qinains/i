#!/usr/bin/env python3
"""Collect upstream notices from the locked Cargo source packages, without executing them."""
import argparse
import base64
import hashlib
import json
from pathlib import Path
import re
import subprocess
import tomllib
from urllib.parse import urlsplit

ROOT = Path(__file__).resolve().parent.parent
FIRST_PARTY = {"i", "i-plugins", "i-wasm-protocol", "i-wasm-package", "i-wasm-sdk"}
SUPPLEMENTS = ROOT / "vendor/licenses"
PUBLIC_REPOSITORIES = {"madsmtm/objc2", "DoumanAsh/clipboard-win", "r-efi/r-efi", "bytecodealliance/wasm-tools"}


def package_key(package):
    return package['name'] + '@' + package['version']


def first_party(package):
    return package['name'] in FIRST_PARTY and not package.get('source') and Path(package['manifest_path']).resolve().is_relative_to(ROOT)


def supplement_files(package):
    index = SUPPLEMENTS / 'index.json'
    records = json.loads(index.read_text()) if index.is_file() else {}
    result = []
    for entry in records.get(package_key(package), {}).get('files', []):
        if not re.fullmatch(r'[a-f0-9]{64}\.txt', entry['file']):
            raise SystemExit('Invalid license supplement filename')
        path = SUPPLEMENTS / entry['file']
        if path.is_symlink() or not path.is_file() or path.stat().st_size > 1024 * 1024 or hashlib.sha256(path.read_bytes()).hexdigest() != entry['sha256']:
            raise SystemExit('License supplement checksum mismatch')
        result.append(path)
    return result


def license_files(package):
    root = Path(package["manifest_path"]).resolve().parent
    candidates = set()
    for path in root.iterdir():
        if path.name.upper().startswith(("LICENSE", "LICENCE", "COPYING", "COPYRIGHT", "NOTICE", "AUTHORS")):
            candidates.update(item for item in path.rglob("*") if item.is_file()) if path.is_dir() else candidates.add(path)
    if package.get("license_file"):
        candidates.add(root / package["license_file"])
    for path in candidates:
        if path.is_symlink() or not path.resolve().is_relative_to(root) or not path.is_file() or path.stat().st_size > 1024 * 1024:
            raise SystemExit("Invalid upstream license file for " + package["name"])
    primary = package.get('license_file') or any(path.name.upper().startswith(('LICENSE', 'LICENCE', 'COPYING')) for path in candidates)
    # r-efi publishes its complete MIT terms in AUTHORS rather than LICENSE.
    inline_mit = any(all(marker in path.read_text(encoding='utf-8') for marker in ['Permission is hereby granted', 'THE SOFTWARE IS PROVIDED', 'COPYRIGHT:']) for path in candidates if path.name.upper().startswith('AUTHORS'))
    supplements = supplement_files(package) if not primary and not inline_mit else []
    candidates.update(supplements)
    if not primary and not inline_mit and not supplements:
        raise SystemExit("Missing upstream license text for " + package["name"])
    return sorted(candidates)


def fetch_missing(packages):
    SUPPLEMENTS.mkdir(exist_ok=True)
    index = SUPPLEMENTS / 'index.json'
    records = json.loads(index.read_text()) if index.is_file() else {}
    cache = {}
    def github(endpoint):
        if endpoint not in cache:
            cache[endpoint] = json.loads(subprocess.check_output(['gh', 'api', endpoint], text=True))
        return cache[endpoint]
    for package in packages:
        if first_party(package):
            continue
        try:
            license_files(package)
            continue
        except SystemExit:
            pass
        if package['name'] in ['winapi-i686-pc-windows-gnu', 'winapi-x86_64-pc-windows-gnu']:
            upstream = next(value for value in packages if value['name'] == 'winapi')
            files = [(path.name, path.read_bytes()) for path in license_files(upstream)]
            provenance = {'source': 'https://crates.io/crates/winapi/' + upstream['version'], 'note': 'Architecture import-library crates share the parent winapi-rs repository notices.'}
        else:
            parsed = urlsplit(package.get('repository') or '')
            repository = '/'.join(parsed.path.strip('/').split('/')[:2])
            if parsed.scheme != 'https' or parsed.hostname != 'github.com' or repository not in PUBLIC_REPOSITORIES:
                raise SystemExit('Review the upstream repository before fetching licenses for ' + package_key(package))
            vcs = json.loads(Path(package['manifest_path']).with_name('.cargo_vcs_info.json').read_text())
            revision = vcs['git']['sha1']
            if not re.fullmatch(r'[a-f0-9]{40}', revision):
                raise SystemExit('Invalid pinned upstream revision')
            tree = github(f'repos/{repository}/git/trees/{revision}')['tree']
            entries = [entry for entry in tree if entry['type'] == 'blob' and entry['path'].upper().startswith(('LICENSE', 'LICENCE', 'COPYING', 'NOTICE'))]
            if not entries:
                raise SystemExit('No root notices at pinned upstream revision for ' + package_key(package))
            files = []
            for entry in entries:
                data = base64.b64decode(github(f'repos/{repository}/git/blobs/{entry["sha"]}')['content'])
                if len(data) > 1024 * 1024 or hashlib.sha1(b'blob ' + str(len(data)).encode() + b'\0' + data).hexdigest() != entry['sha']:
                    raise SystemExit('Upstream license blob verification failed')
                files.append((entry['path'], data))
            provenance = {'repository': 'https://github.com/' + repository, 'revision': revision}
        record = {**provenance, 'files': []}
        for name, data in files:
            digest = hashlib.sha256(data).hexdigest(); filename = digest + '.txt'
            (SUPPLEMENTS / filename).write_bytes(data)
            record['files'].append({'upstream_file': name, 'file': filename, 'sha256': digest})
        records[package_key(package)] = record
        index.write_text(json.dumps(records, indent=2, sort_keys=True) + '\n')
    index.write_text(json.dumps(records, indent=2, sort_keys=True) + '\n')


def render(packages):
    texts = {}; records = []; missing = []
    for package in sorted(packages, key=lambda value: (value["name"], value["version"])):
        if first_party(package):
            continue
        references = []
        try:
            paths = license_files(package)
        except SystemExit:
            missing.append(package["name"] + " " + package["version"])
            continue
        for path in paths:
            value = path.read_text(encoding="utf-8").replace("\r\n", "\n").strip()
            digest = hashlib.sha256(value.encode()).hexdigest()
            texts[digest] = value
            references.append(path.name + " [" + digest + "]")
        records.append(f'{package["name"]} {package["version"]}\nDeclared license: {package.get("license") or "see upstream text"}\nNotices: ' + "; ".join(references))
    if missing:
        raise SystemExit("Missing upstream license texts: " + ", ".join(missing))
    header = "Third-party Rust dependency licenses and notices\n\nFirst-party i / lninl code is MIT licensed. Dependencies retain their own licenses.\nThis is a conservative superset of locked build, test and target dependencies,\nnot a claim that every listed crate is linked into every binary.\nVendored JavaScript notices are separately retained in web/module-lexer.LICENSE\nand web/module-lexer.NOTICE.\n\n"
    return header + "\n\n".join(records) + "\n\n" + "\n\n".join("===== " + key + " =====\n" + value for key, value in sorted(texts.items())) + "\n"


def check_lock_coverage(metadata, lock):
    identity = lambda value: (value['name'], value['version'], value.get('source'))
    expected = {identity(value) for value in lock['package']}
    actual = {identity(value) for value in metadata['packages']}
    missing = expected - actual
    if missing:
        raise SystemExit('License metadata omitted locked packages: ' + ', '.join(sorted(name + '@' + version for name, version, _ in missing)))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    parser.add_argument('--fetch-missing', action='store_true')
    parser.add_argument('--manifest-path', type=Path)
    parser.add_argument('--include-license', type=Path)
    parser.add_argument('--output', type=Path)
    args = parser.parse_args()
    if args.manifest_path and not args.output:
        parser.error('--manifest-path requires an explicit --output; the main notices must not be replaced by a plugin subset')
    if args.include_license and not args.manifest_path:
        parser.error('--include-license requires --manifest-path')
    command = ["cargo", "metadata", "--all-features", "--locked", "--offline", "--format-version", "1"]
    if args.manifest_path:
        command += ['--manifest-path', str(args.manifest_path.resolve())]
    metadata = json.loads(subprocess.check_output(command, cwd=ROOT, text=True))
    check_lock_coverage(metadata, tomllib.loads((Path(metadata['workspace_root']) / 'Cargo.lock').read_text()))
    if args.fetch_missing:
        fetch_missing(metadata['packages'])
    packages = metadata['packages']
    if args.manifest_path:
        packages = [package for package in packages if Path(package['manifest_path']).resolve() != args.manifest_path.resolve()]
    text = render(packages)
    if args.include_license:
        text = args.include_license.read_text(encoding='utf-8') + '\n\n' + text
    if args.manifest_path and any(package['name'] == 'i-wasm-sdk' for package in packages):
        text += '\nHarness WASM SDK license:\n\n' + (ROOT / 'LICENSE').read_text(encoding='utf-8')
    destination = args.output or ROOT / "THIRD_PARTY_LICENSES.txt"
    if args.check:
        if not destination.is_file() or destination.read_text(encoding="utf-8") != text:
            raise SystemExit("Third-party notices are stale; run scripts/build_license_notices.py")
    else:
        destination.write_text(text, encoding="utf-8", newline='\n')
    print(f"Verified {len(metadata['packages'])} locked packages; notices: {len(text.encode())} bytes")


if __name__ == "__main__":
    main()
