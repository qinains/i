import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


class WebsiteTests(unittest.TestCase):
    def test_unlisted_downloads_never_enter_the_public_site(self):
        with tempfile.NamedTemporaryFile(dir=ROOT / "web/downloads", suffix="-private-canary.json") as canary:
            subprocess.run(["python3", str(ROOT / "scripts/build_site.py")], check=True)
            self.assertFalse((ROOT / "dist/site/downloads" / Path(canary.name).name).exists())

    def test_site_build(self):
        subprocess.run(["python3", str(ROOT / "scripts/build_site.py")], check=True)
        output = ROOT / "dist/site"
        for name in ["index.html", "docs/index.html", "docs/plugin-development/index.html", "docs/wasm-plugin-reference/index.html", "sitemap.xml", "search-index.json", "app/pkg/i_bg.wasm"]:
            self.assertTrue((output / name).is_file(), name)
        for name in ["app/tests", "app/node_modules", "docs/superpowers", "app/app.js.map"]:
            self.assertFalse((output / name).exists(), name)
        tutorial = (output / "docs/plugin-development/index.html").read_text()
        self.assertIn("build_wasm_plugin.sh", tutorial)
        self.assertIn("hplugin.zip", tutorial)
        self.assertIn('https://i.lninl.com/docs/plugin-development/', tutorial)
        self.assertNotIn('href="javascript:', tutorial)


if __name__ == "__main__":
    unittest.main()
