import importlib.util
from pathlib import Path
import unittest
import tempfile
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("export_release", Path(__file__).with_name("export_release.py"))
release = importlib.util.module_from_spec(spec)
spec.loader.exec_module(release)


class ExportPolicyTests(unittest.TestCase):
    def test_fresh_named_snapshot_keeps_an_existing_audited_snapshot(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "dist/release-source").mkdir(parents=True)
            audited = root / "dist/release-source/keep.txt"
            audited.write_text("audited")
            (root / "LICENSE").write_text("fixture license")
            with patch.object(release, "ROOT", root), patch.object(release, "DIRECTORIES", set()), patch.object(release, "FILES", {"LICENSE"}):
                release.main(["--destination", "dist/final-source"])
                self.assertEqual((root / "dist/final-source/LICENSE").read_text(), "fixture license")
                self.assertEqual(audited.read_text(), "audited")
                self.assertTrue((root / "dist/final-source-inventory.json").is_file())
                with self.assertRaises(SystemExit):
                    release.main(["--destination", "dist/final-source"])
                with self.assertRaises(SystemExit):
                    release.main(["--destination", "outside"])

    def test_declared_bench_targets_are_included_in_source_releases(self):
        self.assertIn("benches", release.DIRECTORIES)
        self.assertTrue(release.included(Path("benches/benchmarks.rs")))
        self.assertTrue(release.included(Path("benches/performance.rs")))

    def test_native_runtime_and_pty_helper_are_exported_together(self):
        self.assertIn("tools", release.DIRECTORIES)
        for name in ["plugins/runtime/Cargo.toml", "plugins/runtime/src/lib.rs", "tools/test-fixtures/Cargo.toml", "tools/test-fixtures/src/main.rs"]:
            self.assertTrue(release.included(Path(name)), name)

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
        for name in ["LICENSE", "vendor/globset/LICENSE-MIT", "web/module-lexer.LICENSE", "tests/network.rs", "examples/wasm-word-count/src/lib.rs", ".github/workflows/pages.yml"]:
            self.assertTrue(release.included(Path(name)), name)

    def test_generated_protocol_bindings_do_not_enter_source_snapshots(self):
        for name in ["plugins/wasm-protocol/pkg/.gitignore", "plugins/wasm-protocol/pkg/i_wasm_protocol.js", "plugins/wasm-protocol/pkg/i_wasm_protocol_bg.wasm"]:
            self.assertFalse(release.included(Path(name)), name)
        self.assertTrue(release.included(Path("plugins/wasm-protocol/src/lib.rs")))


if __name__ == "__main__":
    unittest.main()
