"""Production acquisition publication boundaries, without network access."""
import importlib.util
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("nano_license", Path(__file__).with_name("license.py"))
license = importlib.util.module_from_spec(spec)
spec.loader.exec_module(license)


class LicenseTests(unittest.TestCase):
    def test_failed_transfer_never_publishes_partial_and_retry_fetches_fresh(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            old = root / "NVIDIA-LICENSE.txt"
            old.write_bytes(b"old-unverified-prefix")
            output = root / "fresh-license.txt"

            def failed(command, **kwargs):
                Path(command[command.index("--output") + 1]).write_bytes(b"partial")
                raise subprocess.CalledProcessError(18, command)

            with patch.object(license.subprocess, "run", side_effect=failed):
                with self.assertRaises(subprocess.CalledProcessError):
                    license.acquire(output)
            self.assertEqual(list(root.iterdir()), [old])

            def complete(command, **kwargs):
                Path(command[command.index("--output") + 1]).write_bytes(b"complete-license")

            with patch.object(license.subprocess, "run", side_effect=complete) as run:
                license.acquire(output)
                run.assert_called_once()
            self.assertEqual(output.read_bytes(), b"complete-license")
            self.assertEqual(old.read_bytes(), b"old-unverified-prefix")
            with patch.object(license.subprocess, "run") as run:
                with self.assertRaises(ValueError):
                    license.acquire(output)
                run.assert_not_called()

    def test_empty_success_refuses_and_dangling_output_is_preserved(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            output = root / "license.txt"
            with patch.object(license.subprocess, "run"):
                with self.assertRaises(ValueError):
                    license.acquire(output)
            self.assertEqual(list(root.iterdir()), [])
            output.symlink_to(root / "absent")
            with patch.object(license.subprocess, "run") as run:
                with self.assertRaises(ValueError):
                    license.acquire(output)
                run.assert_not_called()
            self.assertTrue(output.is_symlink())


if __name__ == "__main__":
    unittest.main()
