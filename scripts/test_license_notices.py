import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('license_notices', Path(__file__).with_name('build_license_notices.py'))
notices = importlib.util.module_from_spec(spec)
spec.loader.exec_module(notices)


class LicenseNoticeTests(unittest.TestCase):
    def package(self, root, name='fixture'):
        return {'name': name, 'version': '1.0.0', 'license': 'MIT', 'manifest_path': str(root / 'Cargo.toml')}

    def test_lowercase_notices_and_authors_are_preserved(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for name in ['license-mit', 'AUTHORS']:
                (root / name).write_text('upstream attribution')
            self.assertEqual([path.name for path in notices.license_files(self.package(root))], ['AUTHORS', 'license-mit'])

    def test_missing_notice_is_not_silently_accepted(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            with patch.object(notices, 'SUPPLEMENTS', root / 'absent'), self.assertRaises(SystemExit):
                notices.license_files(self.package(root))

    def test_authors_without_license_terms_are_not_a_license(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary); (root / 'AUTHORS').write_text('Names only')
            with patch.object(notices, 'SUPPLEMENTS', root / 'absent'), self.assertRaises(SystemExit):
                notices.license_files(self.package(root))

    def test_lock_coverage_rejects_an_omitted_optional_dependency(self):
        package = {'name': 'fixture', 'version': '1.0.0', 'source': 'registry+fixture'}
        with self.assertRaises(SystemExit):
            notices.check_lock_coverage({'packages': []}, {'package': [package]})
        notices.check_lock_coverage({'packages': [package]}, {'package': [package]})

    def test_supplement_bytes_are_checked(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary); crate = root / 'crate'; crate.mkdir()
            data = b'copyright and permission'; digest = hashlib.sha256(data).hexdigest()
            target = root / (digest + '.txt'); target.write_bytes(data)
            (root / 'index.json').write_text(json.dumps({'fixture@1.0.0': {'files': [{'file': target.name, 'sha256': digest}]}}))
            with patch.object(notices, 'SUPPLEMENTS', root):
                self.assertEqual(notices.license_files(self.package(crate)), [target])
                target.write_text('changed')
                with self.assertRaises(SystemExit):
                    notices.license_files(self.package(crate))

    def test_supplement_cannot_name_a_parent_path(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / 'index.json').write_text(json.dumps({'fixture@1.0.0': {'files': [{'file': '../private', 'sha256': '0' * 64}]}}))
            with patch.object(notices, 'SUPPLEMENTS', root), self.assertRaises(SystemExit):
                notices.supplement_files(self.package(root))

    def test_render_is_stable_and_deduplicates_identical_text(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary); (root / 'LICENSE').write_text('shared upstream notice\n')
            packages = [self.package(root, 'b'), self.package(root, 'a')]
            rendered = notices.render(packages)
            self.assertEqual(rendered, notices.render(list(reversed(packages))))
            self.assertEqual(rendered.count('shared upstream notice'), 1)
            self.assertIn('a 1.0.0', rendered)
            self.assertIn('b 1.0.0', rendered)


if __name__ == '__main__':
    unittest.main()
