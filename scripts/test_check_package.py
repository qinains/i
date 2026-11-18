import tempfile
from pathlib import Path
import unittest
import zipfile

from check_package import extract_package


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


if __name__ == '__main__':
    unittest.main()
