#!/usr/bin/env python3
"""Create a source-only release snapshot without copying local Git history."""
import hashlib
import json
from pathlib import Path
import shutil
import tempfile

ROOT = Path(__file__).resolve().parent.parent
DIRECTORIES = {".github", "assets", "completions", "docs", "examples", "extensions", "locales", "plugins", "scripts", "server", "site", "src", "tests", "vendor", "web"}
FILES = {".gitattributes", ".gitignore", "Cargo.toml", "Cargo.lock", "rustfmt.toml", "LICENSE", "README.md", "README.zh-CN.md", "CONTRIBUTING.md", "SECURITY.md"}
SKIP_PARTS = {".git", ".i", ".1k", ".gstack", ".superpowers", "superpowers", "target", "dist", "node_modules", "__pycache__", "test-results", "playwright-report", "xcuserdata"}
SKIP_PATHS = {"examples/plugins.json", "docs/tools-composer-verification.md", "docs/dependency-size-history.md", "web/pkg", "web/plugin-pkg", "web/assets.json", "web/service-worker.js", "web/downloads", "web/icons/icon-192.png", "web/icons/icon-512.png"}


SKIP_PATHS |= {"docs/dependency-audit.md", "docs/dependency-size.md", "docs/size-report.md"}
SKIP_PATHS |= {"web/LICENSE.txt", "web/THIRD_PARTY_LICENSES.txt"}
FILES |= {"THIRD_PARTY_LICENSES.txt"}


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


def main():
    destination = ROOT / "dist/release-source"
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
    (destination.parent / "release-inventory.json").write_text(json.dumps(inventory, indent=2) + "\n")
    print(f"Exported {len(inventory)} source files to {destination}; not yet audited or published")


if __name__ == "__main__":
    main()
