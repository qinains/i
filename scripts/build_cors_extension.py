#!/usr/bin/env python3
"""Create deterministic, source-only browser extension packages."""
import hashlib
import json
from pathlib import Path
import shutil
import struct
import zipfile
import zlib

ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / "extensions/cors-helper"
OUTPUT = ROOT / "web/downloads"
STAGE = ROOT / "dist/cors-helper"
FILES = ["rules.js", "actions.js", "endpoint.js", "background.js", "popup.html", "popup.css", "popup.js", "README.md", "LICENSE"]


def icon(size):
    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))
    rows = bytearray()
    for y in range(size):
        rows.append(0)
        for x in range(size):
            mark = size * 0.42 <= x < size * 0.58 and (size * 0.2 <= y < size * 0.35 or size * 0.45 <= y < size * 0.8)
            rows.extend((80, 160, 255) if mark else (24, 30, 40))
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", size, size, 8, 2, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(rows, 9)) + chunk(b"IEND", b"")


def main():
    OUTPUT.mkdir(parents=True, exist_ok=True)
    manifest = json.loads((SOURCE / "manifest.json").read_text())
    packages = []
    for browser in ["chromium", "firefox", "safari"]:
        folder = STAGE / browser
        folder.mkdir(parents=True, exist_ok=True)
        variant = dict(manifest)
        if browser != "chromium":
            variant.pop("minimum_chrome_version", None)
            variant["background"] = {"scripts": ["background.js"]}
        if browser == "firefox":
            variant["browser_specific_settings"] = {"gecko": {"id": "harness-cors-helper@lninl.local", "strict_min_version": "142.0", "data_collection_permissions": {"required": ["none"]}}}
        for name in FILES:
            shutil.copyfile(ROOT / "LICENSE" if name == "LICENSE" else SOURCE / name, folder / name)
        for size in [48, 128]:
            (folder / f"icon-{size}.png").write_bytes(icon(size))
        (folder / "manifest.json").write_text(json.dumps(variant, indent=2) + "\n")
        name = f"harness-cors-helper-{browser}-{manifest['version']}.zip"
        path = OUTPUT / name
        with zipfile.ZipFile(path, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
            for filename in sorted(FILES + ["manifest.json", "icon-48.png", "icon-128.png"]):
                info = zipfile.ZipInfo(filename, (2026, 1, 1, 0, 0, 0))
                info.compress_type = zipfile.ZIP_DEFLATED
                info.external_attr = 0o100644 << 16
                archive.writestr(info, (folder / filename).read_bytes())
        packages.append({"browser": browser, "file": name, "sha256": hashlib.sha256(path.read_bytes()).hexdigest(), "bytes": path.stat().st_size})
    (OUTPUT / "cors-helper.json").write_text(json.dumps({"version": manifest["version"], "packages": packages}, indent=2) + "\n")
    print(json.dumps(packages, indent=2))


if __name__ == "__main__":
    main()
