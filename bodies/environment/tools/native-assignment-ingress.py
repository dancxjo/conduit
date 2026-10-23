#!/usr/bin/env python3
"""Physical table staging only: no capability, native bind, Body or Play."""
import argparse
import hashlib
import json
from pathlib import Path
import time
import serial

parser = argparse.ArgumentParser()
parser.add_argument('--port', default='/dev/ttyUSB0')
parser.add_argument('--capture', type=Path, required=True)
args = parser.parse_args()
packet = (args.capture / 'uninstalled-assignment-transfer.bin').read_bytes()
canonical = (args.capture / 'uninstalled-assignment.bin').read_bytes()
assert 169 <= len(packet) <= 8192
assert packet[:8] == b'CDAPREF1'
assert packet[44:169] == canonical[:125]
assert int.from_bytes(packet[8:10], 'little') == len(packet)
port = serial.Serial(port=None, baudrate=115200, timeout=.1, write_timeout=2)
port.port = args.port
port.dtr = True
port.rts = True
pending = bytearray()
raw = bytearray()
replies = []


def event(name, timeout=8):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        pending.extend(port.read(8192))
        assert len(pending) <= 65536, 'reply exceeded finite ingress profile'
        while b'\n' in pending:
            line, _, tail = pending.partition(b'\n')
            pending[:] = tail
            raw.extend(line + b'\n')
            try:
                value = json.loads(line)
            except ValueError:
                continue
            if isinstance(value, dict) and value.get('event') == name:
                replies.append(value)
                return value
    raise RuntimeError('no complete ' + name + ' before deadline')


def stage(data):
    port.write(b'assignment-transfer\n')
    event('native-assignment-transfer-ready')
    port.write(data)
    reply = event('native-assignment-stage')
    assert not reply['body_installed']
    assert not reply['native_plan_installed']
    assert not reply['capability_possession_issued']
    return reply


result = {'proof_class': 'physical-assignment-table-staging', 'completed': False,
          'body_start_claimed': False, 'climate_sampling_claimed': False,
          'wire_bytes': len(packet), 'canonical_bytes': len(canonical),
          'wire_sha256': hashlib.sha256(packet).hexdigest()}
try:
    port.open()
    port.write(b'offer\n')
    before = event('native-root-offer')
    assert not before['assignment_staged'], 'fresh boot required; an assignment is already staged'
    for offset, expected in [(12, 'transfer'), (44 + 64, 'assignment'), (44 + 93, 'assignment')]:
        altered = bytearray(packet)
        altered[offset] ^= 1
        reply = stage(altered)
        assert not reply['staged'] and reply['refusal'] == expected, reply
    accepted = stage(packet)
    assert accepted['staged'] and accepted['refusal'] is None, accepted
    assert bytes(accepted['plan']) == canonical[16:32]
    assert bytes(accepted['fragment']) == canonical[32:48]
    duplicate = stage(packet)
    assert not duplicate['staged'] and duplicate['refusal'] == 'already-staged', duplicate
    port.write(b'offer\n')
    after = event('native-root-offer')
    assert after['assignment_staged']
    assert before['advertisement'] == after['advertisement']
    assert not after['body_installed'] and not after['native_plan_installed']
    result.update(completed=True, boot_id=after['advertisement']['boot_id'],
                  root_heap_used_bytes=accepted['root_heap_used_bytes'],
                  root_heap_peak_bytes=accepted['root_heap_peak_bytes'])
finally:
    port.close()
    (args.capture / 'ingress-uart.raw').write_bytes(raw)
    (args.capture / 'ingress-replies.json').write_text(json.dumps(replies, indent=2) + '\n')
    (args.capture / 'ingress-receipt.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result))
