"""Check release version, existing-tag and checkout boundaries without changing Git refs."""
import importlib.util
from pathlib import Path
import subprocess
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("server_release", Path(__file__).with_name("release.py"))
release = importlib.util.module_from_spec(spec)
spec.loader.exec_module(release)

class TagTests(unittest.TestCase):
    def test_exact_existing_tag_matches(self):
        with patch.object(release.subprocess, "check_output", side_effect=["a" * 40 + "\n", "a" * 40 + "\n"]) as git:
            release.validate_tag("0.1.1-alpha.11", "v0.1.1-alpha.11")
        self.assertEqual(git.call_args_list[0].args[0], ["git", "rev-parse", "--verify", "refs/tags/v0.1.1-alpha.11^{commit}"])
        self.assertEqual(git.call_args_list[1].args[0], ["git", "rev-parse", "HEAD"])

    def test_version_mismatch_is_rejected_before_git(self):
        with patch.object(release.subprocess, "check_output") as git:
            with self.assertRaisesRegex(ValueError, "does not match server version"):
                release.validate_tag("0.1.1-alpha.11", "v0.1.1")
            git.assert_not_called()

    def test_checkout_ahead_of_tag_is_rejected(self):
        with patch.object(release.subprocess, "check_output", side_effect=["a" * 40, "b" * 40]):
            with self.assertRaisesRegex(ValueError, "Checkout does not match"):
                release.validate_tag("0.1.1-alpha.11", "v0.1.1-alpha.11")

    def test_missing_tag_is_not_created(self):
        with patch.object(release.subprocess, "check_output", side_effect=subprocess.CalledProcessError(128, "git rev-parse")) as git:
            with self.assertRaises(subprocess.CalledProcessError):
                release.validate_tag("0.1.1-alpha.11", "v0.1.1-alpha.11")
        self.assertEqual(git.call_count, 1)
        self.assertEqual(git.call_args.args[0][1], "rev-parse")

class MigrationTests(unittest.TestCase):
    def test_deployed_migrations_match(self):
        release.validate_migrations()

    def test_checkout_line_ending_conversion_is_rejected(self):
        original = Path.read_bytes
        def normalized(path):
            data = original(path)
            return data.replace(b"\r\n", b"\n") if path.name == "0002_chat.sql" else data
        with patch.object(Path, "read_bytes", normalized):
            with self.assertRaisesRegex(ValueError, "0002_chat.sql"):
                release.validate_migrations()

if __name__ == "__main__":
    unittest.main()
