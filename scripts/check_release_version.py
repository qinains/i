import os
from pathlib import Path
import tomllib


def validate(tag, version):
    expected = f"v{version}"
    if tag != expected:
        raise ValueError(f"release tag must match Cargo.toml: expected {expected!r}, got {tag!r}")


if __name__ == "__main__":
    root = Path(__file__).resolve().parent.parent
    manifest = tomllib.loads((root / "Cargo.toml").read_text())
    validate(os.environ.get("RELEASE_TAG", ""), manifest["package"]["version"])
