#!/usr/bin/env python3
"""Read-only attended output initialization and current software-offer proof."""
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
records = {}


def receive(event, command, seconds, frame_limit, poll=False):
    deadline = time.monotonic() + seconds
    next_ping = 0
    while time.monotonic() < deadline:
        if time.monotonic() >= next_ping:
            port.write(command)  # Only read-only inspection may be polled.
            next_ping = time.monotonic() + 3 if poll else deadline + 1
        incoming = port.read(8192)
        if len(log) + len(incoming) > 192 * 1024:
            raise RuntimeError('readiness log capacity')
        log.extend(incoming)
        pending.extend(incoming)
        if len(pending) > 128 * 1024:
            raise RuntimeError('readiness pending capacity')
        while b'\n' in pending:
            line, _, tail = pending.partition(b'\n')
            pending[:] = tail
            if len(line) > frame_limit:
                raise RuntimeError('readiness frame capacity')
            try:
                value = json.loads(line)
            except ValueError:
                continue
            if isinstance(value, dict):
                if value.get('event') == 'native-root-request-refused':
                    raise RuntimeError('read-only inspection refused')
                if value.get('event') == event:
                    return value
    raise RuntimeError('readiness reply deadline: ' + event)


try:
    port.open()
    before = receive('native-root-offer', b'offer\n', 30, 12 * 1024, poll=True)
    records['readiness'] = before
    if before['advertisement']['host_id'] != 'esp32/b0a732286178':
        raise RuntimeError('unexpected physical MCU')
    if before.get('sampling_software_ready') is not False or before.get('display_software_ready') is not True:
        raise RuntimeError('wrong/refused diagnostic profile: ' + before.get('display_software_refusal', 'unknown'))
    software = receive('native-display-software-offer', b'display-software-offer\n', 12, 64 * 1024)
    records['software-offer'] = software
    after = receive('native-root-offer', b'offer\n', 8, 12 * 1024)
    records['readiness-after'] = after
    if before['advertisement'] != after['advertisement']:
        raise RuntimeError('native availability changed during software inspection')
    for key in ('host_id', 'boot_id', 'offer_generation'):
        if software[key] != before['advertisement'][key]:
            raise RuntimeError('software scope differs: ' + key)
    for value in (before, software, after):
        if value.get('body_installed') is not False or value.get('native_plan_installed') is not False:
            raise RuntimeError('readiness is not an installed Body or Play')
    if software.get('capability_possession_issued') is not False:
        raise RuntimeError('software inspection cannot issue native possession')
    offers = software['capabilities']
    if len(offers) != 3 or len({v['capability_id'] for v in offers}) != 3:
        raise RuntimeError('wrong/duplicate output computation inventory')
    canonical = json.dumps(offers, separators=(',', ':'), ensure_ascii=False).encode()
    if len(canonical) != software['capabilities_bytes']:
        raise RuntimeError('software inventory extent differs')
    for value in (before, after):
        if not (0 < value['native_root_heap_used_bytes'] <= value['native_root_heap_peak_bytes'] <= value['native_root_heap_bytes']):
            raise RuntimeError('invalid Root heap counters')
finally:
    port.close()
    (args.output / 'serial.log').write_bytes(log)
    for name, value in records.items():
        (args.output / (name + '.json')).write_text(json.dumps(value, indent=2) + '\n')
print(json.dumps({'event': 'display-readiness-verified', 'capture': str(args.output),
                  'root_heap_used': after['native_root_heap_used_bytes'],
                  'root_heap_peak': after['native_root_heap_peak_bytes'],
                  'software_offers': len(offers), 'physical_display_delivery': False}))
