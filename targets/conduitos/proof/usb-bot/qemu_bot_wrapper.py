#!/usr/bin/env python3
"""Own one bounded external BOT fixture process for exactly one QEMU run."""
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time
fixture=Path(__file__).resolve().parent
output=Path(os.environ['BOT_DEVICE_OUTPUT']).resolve()
mode=os.environ['BOT_DEVICE_MODE']
output.mkdir(exist_ok=False)
with tempfile.TemporaryDirectory(prefix='bot-',dir='/dev/shm') as temporary:
    socket=Path(temporary)/'device.sock'
    with (output/'device.log').open('xb') as log:
        device=subprocess.Popen([sys.executable,str(fixture/'bot_device.py'),'--fault-mode',mode,
            '--socket',str(socket),'--transcript',str(output/'wire.jsonl'),'--seconds','120'],
            stdout=log,stderr=log)
        try:
            deadline=time.monotonic()+5
            while not socket.exists():
                if device.poll() is not None or time.monotonic()>deadline:
                    raise RuntimeError('fixture failed to listen')
                time.sleep(.02)
            extra=['-device','qemu-xhci,id=protocol-usb,p2=1,p3=0',
                '-chardev',f'socket,id=bot,path={socket}',
                '-device','usb-redir,chardev=bot,bus=protocol-usb.0,port=1,msos-desc=off,streams=off,debug=4']
            result=subprocess.run([os.environ.get('BOT_QEMU','qemu-system-x86_64'),*sys.argv[1:],*extra],check=False)
            raise SystemExit(result.returncode)
        finally:
            if device.poll() is None:
                device.terminate()
                try:device.wait(timeout=3)
                except subprocess.TimeoutExpired:
                    device.kill();device.wait()
