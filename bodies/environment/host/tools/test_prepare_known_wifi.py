"""CPU-only tests: nmcli and terminal secrets are always injected stand-ins."""
import contextlib
import importlib.util
import io
import json
import os
from pathlib import Path
import stat
import tempfile
import types
import unittest
import warnings
from unittest import mock

SPEC = importlib.util.spec_from_file_location("prepare_known_wifi", Path(__file__).with_name("prepare-known-wifi.py"))
helper = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(helper)
UUID = "12345678-abcd-1234-5678-123456789abc"
OTHER_UUID = "12345678-abcd-1234-5678-123456789abd"
# Deliberately synthetic credentials, never values from NetworkManager.
CURRENT = "fixture-current-only"
ADDITIONAL = "fixture-additional-only"


def result(stdout="", code=0):
    return types.SimpleNamespace(returncode=code, stdout=stdout, stderr="not forwarded")


class NetworkFixture:
    def __init__(self, ssid="Current fixture", password=CURRENT, active=None):
        self.ssid = ssid
        self.password = password
        self.active = active if active is not None else f"{UUID}:802-11-wireless\n"
        self.calls = []

    def __call__(self, argv):
        self.calls.append(argv)
        if argv[-1] == "--active":
            return result(self.active)
        if "802-11-wireless.ssid" in argv:
            return result(self.ssid + "\n")
        if "802-11-wireless-security.psk" in argv:
            return result(self.password + "\n") if self.password is not None else result(code=10)
        raise AssertionError("Unexpected NM query")


class Profiles(unittest.TestCase):
    def test_saved_current_and_separate_exact_troys_prompt_are_read_only(self):
        nm = NetworkFixture()
        prompts = []
        document = helper.collect_profiles(["Troys"], nm, lambda prompt: prompts.append(prompt) or ADDITIONAL)
        self.assertEqual(document["schema"], helper.SCHEMA)
        self.assertEqual([p["ssid"] for p in document["profiles"]], ["Current fixture", "Troys"])
        self.assertEqual([p["password"] for p in document["profiles"]], [CURRENT, ADDITIONAL])
        self.assertEqual(len(prompts), 1)
        self.assertIn('"Troys"', prompts[0])
        self.assertNotIn(CURRENT, str(prompts))
        self.assertNotIn(ADDITIONAL, str(prompts))
        for profile in document["profiles"]:
            self.assertEqual(profile["security"], "wpa2-personal")
            self.assertEqual(profile["priority"], 0)
        self.assertEqual(len(nm.calls), 3)
        self.assertEqual(sum("--show-secrets" in call for call in nm.calls), 1)
        for call in nm.calls:
            self.assertEqual(call[0], "nmcli")
            self.assertIn("show", call)
            self.assertTrue(all(word not in call for word in ("modify", "up", "down", "connect", "sudo")))
            self.assertNotIn(CURRENT, call)
            self.assertNotIn(ADDITIONAL, call)
        self.assertEqual(nm.calls[1][-1], UUID)
        self.assertEqual(nm.calls[2][-1], UUID)

    def test_missing_unreadable_or_invalid_saved_psk_prompts_current_then_troys(self):
        for password in (None, "", "--", "x" * 64, "non-ascii-\u00e9"):
            nm = NetworkFixture(password=password)
            prompts = []
            answers = iter([CURRENT, ADDITIONAL])
            document = helper.collect_profiles(["Troys"], nm, lambda prompt: prompts.append(prompt) or next(answers))
            self.assertEqual([p["password"] for p in document["profiles"]], [CURRENT, ADDITIONAL])
            self.assertEqual(len(prompts), 2)
            self.assertIn("Current fixture", prompts[0])
            self.assertIn("Troys", prompts[1])
        nm = NetworkFixture()
        def unavailable(argv):
            if "--show-secrets" in argv:
                raise helper.PreparationError("Read-only NetworkManager query failed")
            return nm(argv)
        self.assertEqual(helper.collect_profiles([], unavailable, lambda _: CURRENT)["profiles"][0]["password"], CURRENT)

    def test_duplicate_ssid_updates_and_four_distinct_profiles_is_the_bound(self):
        document = helper.collect_profiles(["Troys", "Troys"], NetworkFixture(ssid="Troys"), lambda _: ADDITIONAL)
        self.assertEqual(len(document["profiles"]), 1)
        self.assertEqual(document["profiles"][0]["password"], ADDITIONAL)
        document = helper.collect_profiles(["one", "two", "three"], NetworkFixture(), lambda _: ADDITIONAL)
        self.assertEqual(len(document["profiles"]), 4)
        with self.assertRaises(helper.PreparationError):
            helper.collect_profiles(["one", "two", "three", "four"], NetworkFixture(), lambda _: self.fail("No prompt beyond profile bound"))

    def test_exactly_one_active_wifi_uuid_required_before_password_read(self):
        for active in ("", f"{UUID}:ethernet\n", f"{UUID}:wifi\n{OTHER_UUID}:wifi\n", "bad-id:wifi\n"):
            nm = NetworkFixture(active=active)
            with self.assertRaises(helper.PreparationError):
                helper.collect_profiles(["Troys"], nm, lambda _: self.fail("No prompt for ambiguous current network"))
            self.assertEqual(len(nm.calls), 1)

    def test_utf8_ssid_and_printable_ascii_password_bounds(self):
        for ssid in ("a", "x" * 32, "\u00e9" * 16, " space preserved ", "line\nbreak"):
            self.assertEqual(helper.validate_ssid(ssid), ssid)
        for ssid in ("", "x" * 33, "\u00e9" * 17, "embedded\0nul", "\ud800"):
            with self.assertRaises(helper.PreparationError):
                helper.validate_ssid(ssid)
        for password in ("x" * 8, "x" * 63, " space allowed "):
            self.assertEqual(helper.validate_password(password), password)
        for password in ("x" * 7, "x" * 64, "tab\tchar", "newline\npass", "unicode-\u00e9", "\x7f" * 8, None):
            with self.assertRaises(helper.PreparationError):
                helper.validate_password(password)

    def test_cli_never_emits_password_and_refuses_noninteractive_before_queries(self):
        with tempfile.TemporaryDirectory() as directory:
            os.chmod(directory, 0o700)
            output = Path(directory) / "known.json"
            stdout, stderr = io.StringIO(), io.StringIO()
            with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
                status = helper.main(["--add-ssid", "Troys", "--output", str(output)], run=NetworkFixture(), prompt=lambda _: ADDITIONAL, interactive=True)
            self.assertEqual(status, 0)
            self.assertNotIn(CURRENT, stdout.getvalue() + stderr.getvalue())
            self.assertNotIn(ADDITIONAL, stdout.getvalue() + stderr.getvalue())
            with contextlib.redirect_stderr(io.StringIO()):
                self.assertEqual(helper.main([], run=lambda _: self.fail("No NM call without terminal"), prompt=lambda _: self.fail("No prompt without terminal"), interactive=False), 1)

    def test_getpass_refuses_echo_fallback_and_existing_output_refuses_before_queries(self):
        with mock.patch.object(helper.getpass, "getpass", side_effect=lambda *args, **kwargs: warnings.warn("synthetic fallback", helper.getpass.GetPassWarning)):
            with self.assertRaises(helper.PreparationError):
                helper.secure_prompt("Synthetic private prompt: ")
        with tempfile.TemporaryDirectory() as directory:
            os.chmod(directory, 0o700)
            output = Path(directory) / "known.json"
            output.write_bytes(b"previous-private-state")
            output.chmod(0o600)
            with contextlib.redirect_stderr(io.StringIO()):
                status = helper.main(["--output", str(output), "--add-ssid", "Troys"], run=lambda _: self.fail("No NM query without overwrite authorization"), prompt=lambda _: self.fail("No prompt without overwrite authorization"), interactive=True)
            self.assertEqual(status, 1)
            self.assertEqual(output.read_bytes(), b"previous-private-state")


class PasswordFile(unittest.TestCase):
    def test_file_for_single_additional_ssid_prompts_only_missing_current_password(self):
        reads, prompts = [], []
        document = helper.collect_profiles(["Troys"], NetworkFixture(password=None), lambda prompt: prompts.append(prompt) or CURRENT, "/explicit/raw-file", lambda path: reads.append(path) or ADDITIONAL)
        self.assertEqual(reads, ["/explicit/raw-file"])
        self.assertEqual(len(prompts), 1)
        self.assertIn("Current fixture", prompts[0])
        self.assertNotIn("Troys", prompts[0])
        self.assertEqual([p["password"] for p in document["profiles"]], [CURRENT, ADDITIONAL])
        document = helper.collect_profiles(["Another exact SSID"], NetworkFixture(ssid="Another exact SSID", password=None), lambda _: self.fail("File already supplies this SSID"), "raw", lambda _: ADDITIONAL)
        self.assertEqual(len(document["profiles"]), 1)
        self.assertEqual(document["profiles"][0]["password"], ADDITIONAL)
        for additions in ([], ["Troys", "other"]):
            with self.assertRaises(helper.PreparationError):
                helper.collect_profiles(additions, lambda _: self.fail("No query for invalid flag pairing"), password_file="raw", read_password=lambda _: self.fail("No secret read for invalid flag pairing"))

    def test_one_line_terminator_removed_spaces_preserved_and_unsafe_files_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "raw"
            expected = " fixture spaces valid "
            for ending in (b"", b"\n", b"\r", b"\r\n"):
                source.write_bytes(expected.encode("ascii") + ending)
                source.chmod(0o600)
                self.assertEqual(helper.read_password_file(source), expected)
            for data in (b"fixture-password\n\n", b"fixture-password\x00", "fixture-\u00e9".encode("utf-8"), b"x" * 1025):
                source.write_bytes(data)
                with self.assertRaises(helper.PreparationError) as refusal:
                    helper.read_password_file(source)
                self.assertNotIn("fixture-password", str(refusal.exception))
            source.write_bytes(ADDITIONAL.encode("ascii"))
            with self.assertRaises(helper.PreparationError):
                helper.read_password_file(source, uid=os.getuid() + 1)
            source.chmod(0o666)
            with self.assertRaises(helper.PreparationError):
                helper.read_password_file(source)
            source.chmod(0o600)
            link = Path(directory) / "link"
            link.symlink_to(source)
            with self.assertRaises(helper.PreparationError):
                helper.read_password_file(link)
            with self.assertRaises(helper.PreparationError):
                helper.read_password_file(directory)
            fifo = Path(directory) / "fifo"
            os.mkfifo(fifo, 0o600)
            with self.assertRaises(helper.PreparationError):
                helper.read_password_file(fifo)

    def test_cli_password_file_uses_supplied_additional_secret_without_echo(self):
        with tempfile.TemporaryDirectory() as directory:
            os.chmod(directory, 0o700)
            source, output = Path(directory) / "raw", Path(directory) / "known.json"
            source.write_bytes(ADDITIONAL.encode("ascii") + b"\n")
            source.chmod(0o600)
            stdout, stderr, prompts = io.StringIO(), io.StringIO(), []
            with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
                status = helper.main(["--add-ssid", "Troys", "--password-file", str(source), "--output", str(output)], run=NetworkFixture(password=None), prompt=lambda prompt: prompts.append(prompt) or CURRENT, interactive=True)
            self.assertEqual(status, 0)
            self.assertEqual(len(prompts), 1)
            self.assertEqual(json.loads(output.read_text())["profiles"][1]["password"], ADDITIONAL)
            self.assertNotIn(CURRENT, stdout.getvalue() + stderr.getvalue())
            self.assertNotIn(ADDITIONAL, stdout.getvalue() + stderr.getvalue())


class PrivatePublication(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        os.chmod(self.directory.name, 0o700)
        self.output = Path(self.directory.name) / "known.json"
        self.document = helper.collect_profiles(["Troys"], NetworkFixture(), lambda _: ADDITIONAL)

    def test_atomic_private_json_fsyncs_file_before_directory_and_no_clobber(self):
        kinds = []
        real_fsync = os.fsync
        def fsync(descriptor):
            kinds.append(stat.S_IFMT(os.fstat(descriptor).st_mode))
            real_fsync(descriptor)
        with mock.patch.object(helper.os, "fsync", side_effect=fsync):
            helper.write_private(self.document, self.output)
        self.assertEqual(kinds, [stat.S_IFREG, stat.S_IFDIR])
        self.assertEqual(stat.S_IMODE(self.output.stat().st_mode), 0o600)
        self.assertEqual(json.loads(self.output.read_text()), self.document)
        previous = self.output.read_bytes()
        with self.assertRaises(helper.PreparationError):
            helper.write_private({"schema": helper.SCHEMA, "profiles": []}, self.output)
        self.assertEqual(self.output.read_bytes(), previous)
        changed = dict(self.document, profiles=self.document["profiles"][:1])
        helper.write_private(changed, self.output, overwrite=True)
        self.assertEqual(json.loads(self.output.read_text()), changed)
        self.assertEqual(list(self.output.parent.iterdir()), [self.output])

    def test_private_mode_is_exact_even_under_restrictive_callers_umask(self):
        previous = os.umask(0o777)
        try:
            helper.write_private(self.document, self.output)
        finally:
            os.umask(previous)
        self.assertEqual(stat.S_IMODE(self.output.stat().st_mode), 0o600)

    def test_no_overwrite_race_preserves_concurrently_created_output(self):
        rename = helper._rename_noreplace
        def create_then_rename(directory, source, destination):
            self.output.write_bytes(b"concurrent-owned-state")
            self.output.chmod(0o600)
            rename(directory, source, destination)
        with mock.patch.object(helper, "_rename_noreplace", side_effect=create_then_rename):
            with self.assertRaises(helper.PreparationError):
                helper.write_private(self.document, self.output)
        self.assertEqual(self.output.read_bytes(), b"concurrent-owned-state")
        self.assertEqual(list(self.output.parent.iterdir()), [self.output])

    def test_symlink_parent_symlink_file_wrong_owner_and_insecure_existing_are_refused(self):
        target = self.output.parent / "untouched"
        target.write_bytes(b"unique")
        target.chmod(0o600)
        self.output.symlink_to(target)
        for overwrite in (False, True):
            with self.assertRaises(helper.PreparationError):
                helper.write_private(self.document, self.output, overwrite=overwrite)
        self.assertEqual(target.read_bytes(), b"unique")
        self.output.unlink()
        link = self.output.parent / "parent-link"
        link.symlink_to(self.output.parent, target_is_directory=True)
        with self.assertRaises(helper.PreparationError):
            helper.write_private(self.document, link / "known.json")
        with self.assertRaises(helper.PreparationError):
            helper.write_private(self.document, self.output, uid=os.getuid() + 1)
        self.output.write_bytes(b"insecure")
        self.output.chmod(0o644)
        with self.assertRaises(helper.PreparationError):
            helper.write_private(self.document, self.output, overwrite=True)
        self.assertEqual(self.output.read_bytes(), b"insecure")

    def test_file_fsync_failure_preserves_previous_and_removes_private_temporary(self):
        self.output.write_bytes(b"previous")
        self.output.chmod(0o600)
        with mock.patch.object(helper.os, "fsync", side_effect=OSError("synthetic")):
            with self.assertRaises(helper.PreparationError):
                helper.write_private(self.document, self.output, overwrite=True)
        self.assertEqual(self.output.read_bytes(), b"previous")
        self.assertEqual(list(self.output.parent.iterdir()), [self.output])


if __name__ == "__main__":
    unittest.main()
