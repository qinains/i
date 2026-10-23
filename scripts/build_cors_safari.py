#!/usr/bin/env python3
"""Generate a portable macOS Safari project ZIP; optionally compile unsigned."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import shutil
import zipfile

ROOT = Path(__file__).resolve().parent.parent


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--build", action="store_true")
    args = parser.parse_args()
    subprocess.run(["python3", str(ROOT / "scripts/build_cors_extension.py")], check=True)
    version = json.loads((ROOT / "extensions/cors-helper/manifest.json").read_text())["version"]
    parent = ROOT / f"dist/cors-helper/safari-project-{version}"
    if not parent.exists():
        subprocess.run(["xcrun", "safari-web-extension-converter", str(ROOT / "dist/cors-helper/safari"), "--project-location", str(parent), "--app-name", "Harness CORS Helper", "--bundle-identifier", "local.lninl.Harness-CORS-Helper", "--macos-only", "--copy-resources", "--no-open", "--no-prompt"], check=True)
    folder = parent / "Harness CORS Helper"
    resources = folder / "Harness CORS Helper Extension/Resources"
    if resources.is_symlink():
        raise SystemExit("Generated Safari resources must not be a symbolic link")
    if resources.exists():
        shutil.rmtree(resources)
    for source in (ROOT / "dist/cors-helper/safari").rglob("*"):
        if source.is_file():
            target = resources / source.relative_to(ROOT / "dist/cors-helper/safari")
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, target)
    output = ROOT / f"web/downloads/harness-cors-helper-safari-xcode-{version}.zip"
    with zipfile.ZipFile(output, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
        for path in sorted(folder.rglob("*")):
            if path.is_file() and "xcuserdata" not in path.parts and path.name != ".DS_Store":
                info = zipfile.ZipInfo(str(path.relative_to(parent)), (2026, 1, 1, 0, 0, 0))
                info.compress_type = zipfile.ZIP_DEFLATED
                info.external_attr = 0o100644 << 16
                archive.writestr(info, path.read_bytes())
    print(json.dumps({"file": str(output.relative_to(ROOT)), "sha256": hashlib.sha256(output.read_bytes()).hexdigest(), "runtime_verified": False}))
    if args.build:
        subprocess.run(["xcodebuild", "-project", str(folder / "Harness CORS Helper.xcodeproj"), "-scheme", "Harness CORS Helper", "-configuration", "Release", "-derivedDataPath", str(ROOT / "dist/cors-helper/safari-build-final"), "CODE_SIGNING_ALLOWED=NO", "build"], check=True)


if __name__ == "__main__":
    main()
