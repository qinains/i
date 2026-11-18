#!/usr/bin/env python3
"""Create a source-only release snapshot without copying local Git history."""
import hashlib
import argparse
import json
from pathlib import Path
import shutil
import tempfile

ROOT = Path(__file__).resolve().parent.parent
DIRECTORIES = {".github", "assets", "benches", "completions", "docs", "examples", "extensions", "locales", "plugins", "scripts", "server", "site", "src", "tests", "tools", "vendor", "web"}
FILES = {".gitattributes", ".gitignore", "Cargo.toml", "Cargo.lock", "rustfmt.toml", "LICENSE", "README.md", "README.zh-CN.md", "CONTRIBUTING.md", "SECURITY.md"}
SKIP_PARTS = {".git", ".i", ".1k", ".gstack", ".superpowers", "superpowers", "target", "dist", "node_modules", "__pycache__", "test-results", "playwright-report", "xcuserdata"}
SKIP_PATHS = {"examples/plugins.json", "docs/tools-composer-verification.md", "docs/dependency-size-history.md", "web/pkg", "web/plugin-pkg", "web/assets.json", "web/service-worker.js", "web/downloads", "web/icons/icon-192.png", "web/icons/icon-512.png"}


SKIP_PATHS |= {"docs/dependency-audit.md", "docs/dependency-size.md", "docs/size-report.md"}
SKIP_PATHS.add("web/plugins/vendor")
SKIP_PATHS.add("plugins/wasm-protocol/pkg")


def included(relative):
    if any(part in SKIP_PARTS for part in relative.parts):
        return False
    if any(relative == Path(path) or Path(path) in relative.parents for path in SKIP_PATHS):
        return False
    if relative.name == ".DS_Store" or relative.name.startswith(".env"):
        return False
    return not relative.name.endswith((".pyc", ".local.json", ".pem", ".key", ".log", ".swp"))


def source_files(source):
    if source.is_symlink():
        raise SystemExit("Release root must not be a symbolic link: " + source.name)
    return sorted(source.rglob("*")) if source.is_dir() else [source]


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--destination", default="dist/release-source", help="fresh snapshot path inside dist/")
    args = parser.parse_args(argv)
    destination = (ROOT / args.destination).resolve()
    dist = (ROOT / "dist").resolve()
    if destination == dist or not destination.is_relative_to(dist):
        raise SystemExit("Release snapshot must be inside dist/")
    destination.parent.mkdir(exist_ok=True)
    if destination.exists():
        raise SystemExit("Release snapshot already exists; choose a fresh snapshot rather than overwrite an audited tree")
    staging = Path(tempfile.mkdtemp(prefix="release-source-", dir=destination.parent))
    inventory = []
    for name in sorted(FILES | DIRECTORIES):
        source = ROOT / name
        paths = source_files(source)
        for path in paths:
            relative = path.relative_to(ROOT)
            if not included(relative):
                continue
            if path.is_symlink():
                raise SystemExit("Release source must not contain symbolic links: " + str(relative))
            if not path.is_file():
                continue
            target = staging / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(path, target)
            inventory.append({"path": str(relative), "sha256": hashlib.sha256(target.read_bytes()).hexdigest()})
    staging.rename(destination)
    inventory_name = "release-inventory.json" if destination.name == "release-source" else destination.name + "-inventory.json"
    (destination.parent / inventory_name).write_text(json.dumps(inventory, indent=2) + "\n")
    print(f"Exported {len(inventory)} source files to {destination}; not yet audited or published")


if __name__ == "__main__":
    main()
