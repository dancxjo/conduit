#!/usr/bin/env python3
"""Physical output table staging only; no native possession or Body start."""
import argparse
import hashlib
import json
from pathlib import Path
import time
import serial

parser = argparse.ArgumentParser()
parser.add_argument('--capture', type=Path, required=True)
parser.add_argument('--port', default='/dev/ttyUSB0')
args = parser.parse_args()
packet = (args.capture / 'uninstalled-output-assignment-transfer.bin').read_bytes()
assert 169 <= len(packet) <= 8192 and packet[:8] == b'CDAPREF1'
assert int.from_bytes(packet[8:10], 'little') == len(packet)
port = serial.Serial(port=None, baudrate=115200, timeout=.1, write_timeout=2)
port.port, port.dtr, port.rts = args.port, True, True
pending, raw, replies = bytearray(), bytearray(), []


def event(name):
    end = time.monotonic() + 10
    while time.monotonic() < end:
        incoming = port.read(8192)
        raw.extend(incoming)
        pending.extend(incoming)
        assert len(raw) <= 128 * 1024 and len(pending) <= 64 * 1024
        while b'\n' in pending:
            line, _, tail = pending.partition(b'\n')
            pending[:] = tail
            try:
                value = json.loads(line)
            except ValueError:
                continue
            if isinstance(value, dict) and value.get('event') == name:
                replies.append(value)
                return value
    raise RuntimeError('display staging reply deadline: ' + name)


def stage(data):
    port.write(b'assignment-transfer\n')
    event('native-assignment-transfer-ready')
    # Each mutation is sent once; pacing bounds ingress below the UART FIFO.
    for start in range(0, len(data), 32):
        port.write(data[start:start + 32])
        time.sleep(.01)
    reply = event('native-assignment-stage')
    assert not reply['body_installed'] and not reply['native_plan_installed']
    assert not reply['capability_possession_issued']
    return reply


result = {'proof_class': 'physical-display-assignment-staging', 'completed': False,
          'wire_bytes': len(packet), 'wire_sha256': hashlib.sha256(packet).hexdigest()}
try:
    port.open()
    port.write(b'offer\n')
    before = event('native-root-offer')
    assert not before['assignment_staged'] and not before['appliance_kernel_prepared']
    assert before['display_software_ready'] and not before['sampling_software_ready']
    for offset, refusal in [(12, 'display-transfer'), (108, 'display-assignment'),
                            (137, 'display-assignment')]:
        altered = bytearray(packet)
        altered[offset] ^= 1
        reply = stage(altered)
        assert not reply['staged'] and reply['refusal'] == refusal, reply
        assert reply['assignment_seal'] is None
    accepted = stage(packet)
    assert accepted['staged'] and accepted['refusal'] is None
    assert bytes(accepted['plan']) == packet[60:76]
    assert bytes(accepted['fragment']) == packet[76:92]
    assert bytes(accepted['assignment_seal']) == hashlib.sha256(packet).digest()
    duplicate = stage(packet)
    assert not duplicate['staged'] and duplicate['refusal'] == 'display-already-staged'
    assert duplicate['assignment_seal'] == accepted['assignment_seal']
    port.write(b'offer\n')
    after = event('native-root-offer')
    assert after['assignment_staged'] and not after['appliance_kernel_prepared']
    assert before['advertisement'] == after['advertisement']
    bound = after['kernel_additional_preparation_bound']
    assert isinstance(bound, int) and bound > 0
    free = after['native_root_heap_bytes'] - after['native_root_heap_used_bytes']
    result.update(completed=True, host_id=after['advertisement']['host_id'],
                  boot_id=after['advertisement']['boot_id'],
                  root_heap_used_bytes=after['native_root_heap_used_bytes'],
                  root_heap_peak_bytes=after['native_root_heap_peak_bytes'],
                  kernel_additional_preparation_bound=bound,
                  available_root_bytes=free, kernel_bound_fits_now=bound <= free,
                  body_installed=False, capability_possession_issued=False)
finally:
    port.close()
    (args.capture / 'display-staging-serial.log').write_bytes(raw)
    (args.capture / 'display-staging-replies.json').write_text(json.dumps(replies, indent=2) + '\n')
    (args.capture / 'display-staging.json').write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps(result))
