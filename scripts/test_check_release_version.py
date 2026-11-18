import unittest

from check_release_version import validate


class ReleaseVersionTests(unittest.TestCase):
    def test_matching_tags(self):
        for version in ("0.1.0", "1.2.3-rc.1", "1.2.3+build.4"):
            validate(f"v{version}", version)

    def test_rejects_branches_missing_tags_and_version_mismatches(self):
        for tag in ("", "main", "0.1.0", "v0.2.0", "v0.1.0\n"):
            with self.subTest(tag=tag), self.assertRaises(ValueError):
                validate(tag, "0.1.0")


if __name__ == "__main__":
    unittest.main()
