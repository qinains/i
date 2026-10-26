import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

SPEC = importlib.util.spec_from_file_location("translation_builder", Path(__file__).with_name("build_translations.py"))
BUILDER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(BUILDER)


class TranslationTests(unittest.TestCase):
    def fixture(self, directory):
        root = Path(directory)
        (root / "locales").mkdir()
        (root / "locales/languages.json").write_text(json.dumps([{"code": "en", "name": "English", "dir": "ltr"}, {"code": "ar", "name": "العربية", "dir": "rtl"}]))
        for area in ["", "web", "site"]:
            folder = root / "locales" / area
            folder.mkdir(exist_ok=True)
            for code, text in [("en", "Remove {item}: https://api.example/v1"), ("ar", "حذف {item}: https://api.example/v1")]:
                (folder / f"{code}.json").write_text(json.dumps({"key": text}))
        return root

    def test_generation_is_deterministic_and_check_rejects_stale_files(self):
        with tempfile.TemporaryDirectory() as directory:
            root = self.fixture(directory)
            BUILDER.generate(root)
            BUILDER.generate(root, check=True)
            original = (root / "web/translations.js").read_bytes()
            BUILDER.generate(root)
            self.assertEqual(original, (root / "web/translations.js").read_bytes())
            (root / "web/translations.js").write_text("stale")
            with self.assertRaisesRegex(ValueError, "stale"):
                BUILDER.generate(root, check=True)

    def test_missing_keys_placeholders_urls_and_bidi_controls_fail_closed(self):
        for value in [{}, {"key": "حذف"}, {"key": "حذف {item}: https://other.example/v1"}, {"key": "\u202eحذف {item}: https://api.example/v1"}]:
            with self.subTest(value=value), tempfile.TemporaryDirectory() as directory:
                root = self.fixture(directory)
                (root / "locales/web/ar.json").write_text(json.dumps(value))
                with self.assertRaises(ValueError):
                    BUILDER.outputs(root)

    def test_duplicate_keys_are_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = self.fixture(directory)
            (root / "locales/site/ar.json").write_text('{"key":"one","key":"two"}')
            with self.assertRaisesRegex(ValueError, "duplicate"):
                BUILDER.outputs(root)


if __name__ == "__main__":
    unittest.main()
