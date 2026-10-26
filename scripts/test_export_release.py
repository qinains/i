import importlib.util
from pathlib import Path
import unittest
import tempfile

spec = importlib.util.spec_from_file_location("export_release", Path(__file__).with_name("export_release.py"))
release = importlib.util.module_from_spec(spec)
spec.loader.exec_module(release)


class ExportPolicyTests(unittest.TestCase):
    def test_top_level_symlink_is_rejected_before_traversal(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "external").mkdir()
            (root / "assets").symlink_to(root / "external", target_is_directory=True)
            with self.assertRaises(SystemExit):
                release.source_files(root / "assets")

    def test_private_and_generated_files_are_excluded(self):
        for name in ["examples/plugins.json", "docs/superpowers/plan.md", "web/node_modules/package/index.js", "web/pkg/i.wasm", "examples/wasm-word-count/target/file", "web/downloads/private.json", "server/.env", "plugins/key.local.json", "site/__pycache__/x.pyc", ".DS_Store"]:
            self.assertFalse(release.included(Path(name)), name)

    def test_source_tests_and_licenses_are_retained(self):
        for name in ["LICENSE", "THIRD_PARTY_LICENSES.txt", "web/module-lexer.LICENSE", "tests/network.rs", "examples/wasm-word-count/src/lib.rs", ".github/workflows/pages.yml"]:
            self.assertTrue(release.included(Path(name)), name)


if __name__ == "__main__":
    unittest.main()
