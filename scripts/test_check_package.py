import tempfile
from pathlib import Path
import unittest
import zipfile
from unittest.mock import Mock, patch

from check_package import extract_package, relocate_bundle


class PackageExtractionTests(unittest.TestCase):
    def test_zip_package(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            archive = root / 'bundle.zip'
            with zipfile.ZipFile(archive, 'w') as package:
                package.writestr('bundle/bin/i.exe', b'fixture')
            extract_package(archive, root / 'out')
            self.assertEqual((root / 'out/bundle/bin/i.exe').read_bytes(), b'fixture')

    def test_zip_rejects_unsafe_paths_and_symlinks(self):
        for name in ('../escape', '/absolute', 'C:/absolute', 'bundle/../../escape',
                     r'bundle\..\escape', 'bundle/link'):
            with self.subTest(name=name), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                archive = root / 'bundle.zip'
                with zipfile.ZipFile(archive, 'w') as package:
                    info = zipfile.ZipInfo(name)
                    if name == 'bundle/link':
                        info.external_attr = 0o120777 << 16
                    package.writestr(info, b'../escape')
                with self.assertRaises(ValueError):
                    extract_package(archive, root / 'out')

    def test_zip_rejects_raw_backslashes_before_windows_normalization(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            archive = root / 'bundle.zip'
            with zipfile.ZipFile(archive, 'w') as package:
                package.writestr('bundle/bin/i.exe', b'fixture')
            archive.write_bytes(archive.read_bytes().replace(b'bundle/bin/i.exe', b'bundle\\bin\\i.exe'))
            with patch.object(zipfile.os, 'sep', '\\'):
                with self.assertRaises(ValueError):
                    extract_package(archive, root / 'out')

    def test_relocation_waits_for_transient_windows_file_access(self):
        locked = PermissionError('in use')
        locked.winerror = 5
        source = Mock()
        source.rename.side_effect = [locked, None]
        with patch('check_package.time.monotonic', return_value=0), patch('check_package.time.sleep') as sleep:
            try:
                relocate_bundle(source, 'moved')
            except PermissionError:
                self.fail('Transient Windows file access was not retried')
        self.assertEqual(source.rename.call_count, 2)
        source.rename.assert_called_with('moved')
        sleep.assert_called_once_with(0.05)

    def test_relocation_does_not_retry_unrelated_errors(self):
        source = Mock()
        source.rename.side_effect = FileNotFoundError('missing')
        with self.assertRaises(FileNotFoundError):
            relocate_bundle(source, 'moved')
        source.rename.assert_called_once_with('moved')

    def test_relocation_windows_file_access_wait_has_a_deadline(self):
        locked = PermissionError('in use')
        locked.winerror = 32
        source = Mock()
        source.rename.side_effect = locked
        with patch('check_package.time.monotonic', side_effect=[0, 3]), patch('check_package.time.sleep') as sleep:
            with self.assertRaises(PermissionError):
                relocate_bundle(source, 'moved')
        source.rename.assert_called_once_with('moved')
        sleep.assert_not_called()


if __name__ == '__main__':
    unittest.main()
