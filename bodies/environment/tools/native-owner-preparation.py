#!/usr/bin/env python3
"""Carry signed control frames while the real Rust Body owner remains alive."""
import argparse
import json
from pathlib import Path
import selectors
import subprocess
import time
import serial

parser = argparse.ArgumentParser()
parser.add_argument('--capture', type=Path, required=True)
parser.add_argument('--admission', type=Path, required=True)
parser.add_argument('--keys', type=Path, required=True)
parser.add_argument('--owner', type=Path, required=True)
args = parser.parse_args()
port = serial.Serial(port=None, baudrate=115200, timeout=.1, write_timeout=2)
port.port = '/dev/ttyUSB0'
port.dtr = True
port.rts = True
pending = bytearray()
replies = []
process = None


def event(name):
    deadline = time.monotonic() + 12
    while time.monotonic() < deadline:
        pending.extend(port.read(8192))
        if len(pending) > 65536:
            raise RuntimeError('UART ingress bound')
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
    raise RuntimeError('deadline: ' + name)


def host_line():
    with selectors.DefaultSelector() as selected:
        selected.register(process.stdout, selectors.EVENT_READ)
        if not selected.select(12):
            raise RuntimeError('owner response deadline')
        line = process.stdout.readline(8193)
    if len(line) > 8192 or not line.endswith('\n'):
        raise RuntimeError('owner frame bound')
    return json.loads(line)


receipt = {'completed': False, 'proof_class': 'physical-signed-native-preparation',
           'body_started': False, 'climate_sampling_claimed': False}
try:
    wire = (args.capture / 'uninstalled-assignment-transfer.bin').read_bytes()
    if not 0 < len(wire) <= 8192:
        raise RuntimeError('transfer bound')
    port.open()
    with (args.capture / 'native-owner-stderr.log').open('w') as errors:
        process = subprocess.Popen([str(args.owner.resolve()), str(args.capture.resolve()),
                                    str(args.admission.resolve()), str(args.keys.resolve())],
                                   stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=errors, text=True)
        for operation in ['Inspect', 'Reserve', 'Prepare']:
            frame = host_line()
            assert frame['event'] == 'owner-command' and frame['request']['operation'] == operation
            encoded = json.dumps(frame['request'], separators=(',', ':')).encode()
            signature = bytes(frame['signature'])
            assert len(encoded) <= 2048 and len(signature) == 64
            port.write(b'owner-control\n')
            event('native-owner-control-ready')
            port.write(len(encoded).to_bytes(2, 'little') + encoded + signature)
            if operation == 'Prepare':
                event('native-owner-assignment-ready')
                port.write(len(wire).to_bytes(2, 'little') + wire)
            reply = event('native-owner-control')
            process.stdin.write(json.dumps(reply) + '\n')
            process.stdin.flush()
        receipt = host_line()
        assert receipt['event'] == 'owner-preparation-complete' and receipt['completed']
        process.stdin.close()
        if process.wait(timeout=8) != 0:
            raise RuntimeError('retained owner refused preparation')
    port.write(b'offer\n')
    after = event('native-root-offer')
    assert after['appliance_kernel_prepared'] and not after['assignment_staged']
    assert not after['body_installed'] and not after['native_plan_installed']
    receipt.update(proof_class='physical-signed-native-preparation',
                   root_heap_used_bytes=after['native_root_heap_used_bytes'],
                   root_heap_peak_bytes=after['native_root_heap_peak_bytes'],
                   native_root_heap_bytes=after['native_root_heap_bytes'])
finally:
    if port.is_open:
        try:
            port.write(b'offer\n')
            observed = event('native-root-offer')
            receipt.update(root_heap_used_bytes=observed['native_root_heap_used_bytes'],
                           root_heap_peak_bytes=observed['native_root_heap_peak_bytes'],
                           native_root_heap_bytes=observed['native_root_heap_bytes'])
        except (RuntimeError, OSError, KeyError):
            receipt['final_root_observation_unavailable'] = True
    port.close()
    if process is not None and process.poll() is None:
        process.terminate()
        process.wait(timeout=3)
    (args.capture / 'native-owner-replies.json').write_text(json.dumps(replies, indent=2) + '\n')
    (args.capture / 'native-owner-receipt.json').write_text(json.dumps(receipt, indent=2) + '\n')
    print(json.dumps(receipt))
