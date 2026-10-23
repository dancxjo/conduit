#!/usr/bin/env python3
"""Physical dormant-kernel preparation; no native possession or Play."""
import argparse
import json
from pathlib import Path
import time
import serial

parser = argparse.ArgumentParser()
parser.add_argument('--capture', type=Path, required=True)
parser.add_argument('--before-staging', action='store_true')
parser.add_argument('--port', default='/dev/ttyUSB0')
args = parser.parse_args()
port = serial.Serial(port=None, baudrate=115200, timeout=.1, write_timeout=2)
port.port = args.port
port.dtr = True
port.rts = True
pending = bytearray()
replies = []


def event(name):
    deadline = time.monotonic() + 8
    while time.monotonic() < deadline:
        pending.extend(port.read(8192))
        if len(pending) > 65536:
            raise RuntimeError('finite UART ingress exceeded')
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
    raise RuntimeError('no complete ' + name + ' before deadline')


def request(command, reply):
    port.write(command.encode() + b'\n')
    return event(reply)


result = {'proof_class': 'physical-dormant-kernel-preparation', 'completed': False,
          'before_staging': args.before_staging, 'body_start_claimed': False,
          'native_possession_claimed': False, 'climate_sampling_claimed': False}
try:
    port.open()
    before = request('offer', 'native-root-offer')
    assert not before['appliance_kernel_prepared']
    assert before['assignment_staged'] != args.before_staging
    reply = request('kernel-prepare', 'native-kernel-preparation')
    assert not reply['body_installed'] and not reply['native_plan_installed']
    assert not reply['capability_possession_issued']
    if args.before_staging:
        assert not reply['prepared'] and reply['refusal'] == 'unavailable', reply
    else:
        assert reply['prepared'] and reply['refusal'] is None, reply
        duplicate = request('kernel-prepare', 'native-kernel-preparation')
        assert not duplicate['prepared'] and duplicate['refusal'] == 'already-prepared'
        assert duplicate['root_heap_used_bytes'] == reply['root_heap_used_bytes']
        # Transferred software must not be re-advertised as a second ready set.
        request('software-offer', 'native-root-request-refused')
    after = request('offer', 'native-root-offer')
    assert before['advertisement'] == after['advertisement']
    assert after['appliance_kernel_prepared'] != args.before_staging
    assert not after['assignment_staged']
    assert not after['body_installed'] and not after['native_plan_installed']
    result.update(completed=True, boot_id=after['advertisement']['boot_id'],
                  kernel_bytes=reply['kernel_bytes'],
                  additional_preparation_bound=reply['additional_preparation_bound'],
                  root_heap_used_bytes=reply['root_heap_used_bytes'],
                  root_heap_peak_bytes=reply['root_heap_peak_bytes'],
                  native_root_heap_bytes=after['native_root_heap_bytes'])
finally:
    port.close()
    prefix = 'kernel-before-staging' if args.before_staging else 'kernel-preparation'
    (args.capture / (prefix + '-replies.json')).write_text(json.dumps(replies, indent=2) + '\n')
    (args.capture / (prefix + '-receipt.json')).write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result))
