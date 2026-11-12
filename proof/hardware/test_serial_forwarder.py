"""Isolated PTY PROOF tests. USB mapping and modem evidence are mocked explicitly.
No native physical identity, membership, Start or authority is established.
"""
import errno
import json
import os
from pathlib import Path
import select
import subprocess
import sys
import tempfile
import threading
import time
import unittest

HERE = Path(__file__).resolve().parent


def read_exact(fd, count):
    result = bytearray()
    deadline = time.monotonic()+5
    while len(result) < count:
        if time.monotonic() >= deadline:
            raise TimeoutError("PTY fixture read deadline")
        if not select.select([fd], [], [], max(0, deadline-time.monotonic()))[0]:
            continue
        chunk = os.read(fd, count-len(result))
        if not chunk:
            raise EOFError("PTY fixture ended early")
        result.extend(chunk)
    return bytes(result)


class ForwarderTests(unittest.TestCase):
    def start(self, extra=(), keep_slave=False):
        master, slave = os.openpty()
        path = os.ttyname(slave)
        if keep_slave:
            self.addCleanup(os.close, slave)
        else:
            os.close(slave)
        # The native lock/raw/framing path is real. PTYs have neither USB sysfs
        # facts nor modem pins; only those evidence observations are mocked.
        bootstrap = ("import sys;sys.path.insert(0,"+repr(str(HERE))+
                     ");import serial_forwarder as f;"
                     "f.usb_mapping=lambda _: {'fixture':'mocked-PTY-not-USB'};"
                     "f.modem=lambda _:0;sys.exit(f.main())")
        child = subprocess.Popen([sys.executable, "-c", bootstrap, "--device", path,
                                  "--idle-seconds", "2", "--eof-seconds", "0.2", *extra],
                                 stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                 stderr=subprocess.PIPE)
        self.addCleanup(lambda: child.kill() if child.poll() is None else None)
        self.addCleanup(lambda: os.close(master) if master >= 0 else None)
        self.addCleanup(child.stdin.close)
        self.addCleanup(child.stdout.close)
        self.addCleanup(child.stderr.close)
        return child, master, path

    def finish(self, child, expected=0):
        self.assertEqual(child.wait(timeout=5), expected)
        records = [json.loads(line) for line in child.stderr.read().splitlines()]
        self.assertTrue(records)
        self.assertTrue(all(not r["native_line_ready"] and not r["execution_authority"]
                            for r in records))
        return records

    def test_exact_duplex_bytes_backpressure_and_future_open_exclusion(self):
        child, master, path = self.start()
        self.assertEqual(read_exact(child.stdout.fileno(), 6), b"READY\n")
        with self.assertRaises(OSError) as error:
            os.open(path, os.O_RDWR | os.O_NOCTTY | os.O_NONBLOCK)
        self.assertEqual(error.exception.errno, errno.EBUSY)
        outgoing = bytes(range(256))*256
        incoming = outgoing[::-1]
        captured = []
        failures = []

        def collect():
            try:
                time.sleep(0.05)  # Exercise finite pending-buffer backpressure.
                captured.append(read_exact(master, len(outgoing)))
            except Exception as exc:
                failures.append(exc)

        def send_client():
            try:
                child.stdin.write(outgoing)
                child.stdin.close()
            except Exception as exc:
                failures.append(exc)

        def send_device():
            try:
                offset = 0
                while offset < len(incoming):
                    offset += os.write(master, incoming[offset:offset+4096])
            except Exception as exc:
                failures.append(exc)

        threads = [threading.Thread(target=f, daemon=True)
                   for f in (collect, send_client, send_device)]
        for thread in threads:
            thread.start()
        self.assertEqual(read_exact(child.stdout.fileno(), len(incoming)), incoming)
        for thread in threads:
            thread.join(timeout=5)
            self.assertFalse(thread.is_alive())
        self.assertFalse(failures)
        self.assertEqual(captured, [outgoing])
        records = self.finish(child)
        done = next(r for r in records if r["event"] == "finished")
        self.assertEqual(done["client_to_tty"], len(outgoing))
        self.assertEqual(done["tty_to_client"], len(incoming))
        # Cleanup explicitly releases TIOCEXCL even while our PTY master lives.
        reopened = os.open(path, os.O_RDWR | os.O_NOCTTY | os.O_NONBLOCK)
        os.close(reopened)

    def test_startup_bytes_are_private_quarantine_not_forwarded_or_acknowledged(self):
        with tempfile.TemporaryDirectory() as folder:
            capture = Path(folder)/"startup.bin"
            child, master, _ = self.start(["--startup-capture", str(capture),
                                          "--startup-seconds", "0.2"])
            opened = json.loads(child.stderr.readline())
            self.assertEqual(opened["event"], "opened")
            os.write(master, b"boot-ROM-evidence\x00\xff")
            self.assertEqual(read_exact(child.stdout.fileno(), 6), b"READY\n")
            child.stdin.close()
            records = self.finish(child)
            self.assertEqual(capture.read_bytes(), b"boot-ROM-evidence\x00\xff")
            self.assertEqual(capture.stat().st_mode & 0o777, 0o600)
            quarantine = next(r for r in records if r["event"] == "startup-quarantine")
            self.assertFalse(quarantine["delivered"])
            self.assertFalse(quarantine["acknowledged"])
            self.assertEqual(child.stdout.read(), b"")

    def test_existing_capture_refuses_before_marker_and_keeps_original_evidence(self):
        with tempfile.TemporaryDirectory() as folder:
            capture = Path(folder)/"startup.bin"
            capture.write_bytes(b"original")
            child, _, _ = self.start(["--startup-capture", str(capture)])
            records = self.finish(child, 1)
            self.assertEqual(child.stdout.read(), b"")
            self.assertEqual(capture.read_bytes(), b"original")
            self.assertTrue(any(r["event"] == "refused" for r in records))

    def test_visible_preexisting_owner_refuses_before_available_marker(self):
        child, _, _ = self.start(keep_slave=True)
        records = self.finish(child, 1)
        self.assertEqual(child.stdout.read(), b"")
        refusal = next(r for r in records if r["event"] == "refused")
        self.assertIn("preexisting competing", refusal["error"])

    def test_startup_extent_refuses_without_consuming_unbounded_input(self):
        with tempfile.TemporaryDirectory() as folder:
            capture = Path(folder)/"startup.bin"
            child, master, _ = self.start(["--startup-capture", str(capture),
                                          "--startup-max-bytes", "4"])
            self.assertEqual(json.loads(child.stderr.readline())["event"], "opened")
            os.write(master, b"12345678")
            records = self.finish(child, 1)
            self.assertEqual(child.stdout.read(), b"")
            self.assertEqual(capture.read_bytes(), b"1234")
            self.assertTrue(any(r["event"] == "refused" for r in records))

    def test_idle_error_releases_original_descriptor_without_retry(self):
        child, _, path = self.start(["--idle-seconds", "0.1"])
        self.assertEqual(read_exact(child.stdout.fileno(), 6), b"READY\n")
        records = self.finish(child, 1)
        self.assertEqual(sum(r["event"] == "opened" for r in records), 1)
        self.assertTrue(any(r["event"] == "refused" for r in records))
        reopened = os.open(path, os.O_RDWR | os.O_NOCTTY | os.O_NONBLOCK)
        os.close(reopened)


if __name__ == "__main__":
    unittest.main()
