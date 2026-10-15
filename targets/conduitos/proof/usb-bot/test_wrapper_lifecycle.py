"""Linux probe-process ownership, separate from native USB/DMA proof."""
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import unittest

FIXTURE = Path(__file__).resolve().parent


def process(pid):
    try:
        fields = Path(f'/proc/{pid}/stat').read_text().rsplit(')', 1)[1].split()
        return fields[0], fields[19]
    except FileNotFoundError:
        return None


def stopped(pid, lease):
    current = process(pid)
    return current is None or current[1] != lease or current[0] == 'Z'


@unittest.skipUnless(sys.platform == 'linux' and shutil.which('qemu-system-x86_64'),
                     'requires Linux /proc and actual QEMU')
class WrapperLifecycle(unittest.TestCase):
    def test_killed_wrapper_stops_its_actual_emulator_and_device(self):
        children = []
        socket = None
        with tempfile.TemporaryDirectory(prefix='bot-owner-test-') as temporary:
            output = Path(temporary) / 'device'
            env = dict(os.environ, BOT_DEVICE_OUTPUT=str(output), BOT_DEVICE_MODE='none',
                       BOT_QEMU=shutil.which('qemu-system-x86_64'), PYTHONDONTWRITEBYTECODE='1')
            with (Path(temporary) / 'wrapper.log').open('wb') as log:
                wrapper = subprocess.Popen([
                    sys.executable, str(FIXTURE / 'qemu_bot_wrapper.py'), '-M', 'q35',
                    '-nodefaults', '-display', 'none', '-monitor', 'none', '-serial', 'none',
                    '-m', '32M', '-S'], env=env, stdout=log, stderr=log)
                try:
                    deadline = time.monotonic() + 5
                    qemu_ready = False
                    while time.monotonic() < deadline:
                        self.assertIsNone(wrapper.poll(), 'wrapper must run before cancellation')
                        owned = Path(f'/proc/{wrapper.pid}/task/{wrapper.pid}/children').read_text().split()
                        if len(owned) == 2:
                            children = [(int(pid), process(int(pid))[1]) for pid in owned]
                            qemu_ready = False
                            for pid, _ in children:
                                qemu_ready |= os.readlink(f'/proc/{pid}/exe').endswith('/qemu-system-x86_64')
                                cmd = Path(f'/proc/{pid}/cmdline').read_bytes().split(b'\0')
                                if b'--socket' in cmd:
                                    socket = Path(os.fsdecode(cmd[cmd.index(b'--socket') + 1]))
                            if qemu_ready and socket and socket.exists():
                                break
                        time.sleep(.02)
                    self.assertEqual(len(children), 2, 'actual emulator and external device required')
                    self.assertTrue(socket and socket.exists())
                    self.assertTrue(qemu_ready, 'actual QEMU must have execed before cancellation')
                    wrapper.kill()  # Matches protocol-run's forced wrapper termination.
                    wrapper.wait(timeout=3)
                    deadline = time.monotonic() + 3
                    while time.monotonic() < deadline and not all(stopped(*c) for c in children):
                        time.sleep(.02)
                    self.assertTrue(all(stopped(*c) for c in children),
                                    'wrapper death must not orphan its emulator/device')
                finally:
                    if wrapper.poll() is None:
                        wrapper.kill()
                        wrapper.wait(timeout=3)
                    for pid, lease in children:
                        if not stopped(pid, lease):
                            os.kill(pid, signal.SIGKILL)
                    if socket and socket.parent.parent == Path('/dev/shm') and socket.parent.name.startswith('bot-'):
                        shutil.rmtree(socket.parent, ignore_errors=True)


if __name__ == '__main__':
    unittest.main()
