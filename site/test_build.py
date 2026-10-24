import subprocess
import tempfile
import unittest
import struct
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


class WebsiteTests(unittest.TestCase):
    def test_brand_icons_share_geometry_and_colors(self):
        self.assertEqual((ROOT / 'site/assets/favicon.svg').read_bytes(), (ROOT / 'web/icons/icon.svg').read_bytes())
        for source in ['web/icons/icon.svg', 'assets/lninl.svg', 'site/assets/site.css', 'web/style.css']:
            self.assertIn('#22d3ee', (ROOT / source).read_text(), source)
        for size in [192, 512]:
            data = (ROOT / f'web/icons/icon-{size}.png').read_bytes()
            position, compressed = 8, b''
            while position < len(data):
                length = struct.unpack('>I', data[position:position+4])[0]
                if data[position+4:position+8] == b'IDAT':
                    compressed += data[position+8:position+8+length]
                position += length+12
            pixels = zlib.decompress(compressed)
            for x, y, expected in [(0, 0, bytes([15, 23, 42])), (size//2, size//2, bytes([34, 211, 238]))]:
                offset = y*(size*3+1)+1+x*3
                self.assertEqual(pixels[offset:offset+3], expected)

    def test_unlisted_downloads_never_enter_the_public_site(self):
        with tempfile.NamedTemporaryFile(dir=ROOT / "web/downloads", suffix="-private-canary.json") as canary:
            subprocess.run(["python3", str(ROOT / "scripts/build_site.py")], check=True)
            self.assertFalse((ROOT / "dist/site/downloads" / Path(canary.name).name).exists())

    def test_site_build(self):
        subprocess.run(["python3", str(ROOT / "scripts/build_site.py")], check=True)
        output = ROOT / "dist/site"
        self.assertEqual((output / "third-party-licenses.txt").read_bytes(), (ROOT / "THIRD_PARTY_LICENSES.txt").read_bytes())
        self.assertTrue((output / "app/THIRD_PARTY_LICENSES.txt").is_file())
        for name in ["index.html", "docs/index.html", "docs/plugin-development/index.html", "docs/wasm-plugin-reference/index.html", "sitemap.xml", "search-index.json", "app/pkg/i_bg.wasm"]:
            self.assertTrue((output / name).is_file(), name)
        for name in ["app/tests", "app/node_modules", "docs/superpowers", "app/app.js.map"]:
            self.assertFalse((output / name).exists(), name)
        tutorial = (output / "docs/plugin-development/index.html").read_text()
        self.assertIn('<article lang="zh-CN">', tutorial)
        self.assertIn('<article lang="en">', (output / 'docs/overview/index.html').read_text())
        self.assertIn('<article lang="en">', (output / 'license/index.html').read_text())
        self.assertIn("build_wasm_plugin.sh", tutorial)
        self.assertIn("hplugin.zip", tutorial)
        self.assertIn('https://i.lninl.com/docs/plugin-development/', tutorial)
        self.assertNotIn('href="javascript:', tutorial)


if __name__ == "__main__":
    unittest.main()
