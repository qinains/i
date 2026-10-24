import importlib.util
import io
from pathlib import Path
import tarfile
import tempfile
import unittest
import zipfile

spec = importlib.util.spec_from_file_location('check_package', Path(__file__).with_name('check_package.py'))
checker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(checker)


class ArchiveTests(unittest.TestCase):
    def test_zip_and_tar_gzip_have_the_same_bundle_layout(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for extension in ['zip', 'tar.gz']:
                archive = root / ('bundle.' + extension)
                if extension == 'zip':
                    with zipfile.ZipFile(archive, 'w') as package:
                        package.writestr('bundle/bin/i.exe', b'fixture')
                else:
                    with tarfile.open(archive, 'w:gz') as package:
                        info = tarfile.TarInfo('bundle/bin/i.exe')
                        info.size = 7
                        package.addfile(info, io.BytesIO(b'fixture'))
                output = root / extension
                output.mkdir()
                bundle = checker.extract_package(archive, output)
                self.assertEqual((bundle / 'bin/i.exe').read_bytes(), b'fixture')

    def test_zip_rejects_escape_symlinks_and_ambiguous_roots(self):
        for name in ['../escape', '/escape', 'C:/escape', 'bundle/../escape', 'bundle\\escape']:
            with self.subTest(name=name), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                archive = root / 'bad.zip'
                with zipfile.ZipFile(archive, 'w') as package:
                    package.writestr(name, 'bad')
                output = root / 'out'
                output.mkdir()
                with self.assertRaises(ValueError):
                    checker.extract_package(archive, output)
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            archive = root / 'bad.zip'
            link = zipfile.ZipInfo('bundle/link')
            link.create_system = 3
            link.external_attr = 0o120777 << 16
            with zipfile.ZipFile(archive, 'w') as package:
                package.writestr(link, '../../escape')
            output = root / 'out'
            output.mkdir()
            with self.assertRaises(ValueError):
                checker.extract_package(archive, output)


if __name__ == '__main__':
    unittest.main()
