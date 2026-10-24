#!/usr/bin/env python3
"""Read-only attended MCU output-software initialization/heap proof."""
import argparse
import json
from pathlib import Path
import time
import serial

parser = argparse.ArgumentParser()
parser.add_argument('--output', type=Path, required=True)
parser.add_argument('--port', default='/dev/ttyUSB0')
args = parser.parse_args()
args.output.mkdir(parents=True, exist_ok=True)
port = serial.Serial(port=None, baudrate=115200, timeout=.1, write_timeout=2)
port.port = args.port
port.dtr = True
port.rts = True
pending = bytearray()
log = bytearray()
result = None
try:
    port.open()
    deadline = time.monotonic() + 30
    ping = 0
    while time.monotonic() < deadline:
        if time.monotonic() >= ping:
            port.write(b'offer\n')  # Readiness inspection only; no mutation retry.
            ping = time.monotonic() + 3
        incoming = port.read(8192)
        if len(log) + len(incoming) > 65536:
            raise RuntimeError('readiness log capacity')
        log.extend(incoming)
        pending.extend(incoming)
        if len(pending) > 24 * 1024:
            raise RuntimeError('readiness frame capacity')
        while b'\n' in pending:
            line, _, tail = pending.partition(b'\n')
            pending[:] = tail
            if len(line) > 12 * 1024:
                raise RuntimeError('readiness frame capacity')
            try:
                value = json.loads(line)
            except ValueError:
                continue
            if isinstance(value, dict) and value.get('event') == 'native-root-offer':
                result = value
                break
        if result is not None:
            break
finally:
    port.close()
    (args.output / 'serial.log').write_bytes(log)
    if result is not None:
        (args.output / 'readiness.json').write_text(json.dumps(result, indent=2) + '\n')
if result is None:
    raise RuntimeError('no readiness reply within 30 seconds')
if result['advertisement']['host_id'] != 'esp32/b0a732286178':
    raise RuntimeError('unexpected physical MCU')
if result.get('sampling_software_ready') is not False:
    raise RuntimeError('wrong diagnostic profile')
if result.get('body_installed') is not False or result.get('native_plan_installed') is not False:
    raise RuntimeError('readiness is not an installed Body or Play')
if result.get('display_software_ready') is not True:
    raise RuntimeError('output initialization refused: ' + result.get('display_software_refusal', 'unknown'))
if not (0 < result['native_root_heap_used_bytes'] <= result['native_root_heap_peak_bytes'] <= result['native_root_heap_bytes']):
    raise RuntimeError('invalid Root heap counters')
print(json.dumps({'event': 'display-readiness-verified', 'capture': str(args.output),
                  'root_heap_used': result['native_root_heap_used_bytes'],
                  'root_heap_peak': result['native_root_heap_peak_bytes'],
                  'physical_display_delivery': False}))
