#!/usr/bin/env python3
"""Carry signed control frames while the real Rust Body owner remains alive."""
import argparse
import hashlib
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
parser.add_argument('--display', action='store_true')
parser.add_argument('--http-fixture', action='store_true')
parser.add_argument('--mode', choices=['prepare', 'start-held', 'run'], default='prepare')
args = parser.parse_args()
port = serial.Serial(port=None, baudrate=115200, timeout=.1, write_timeout=2)
port.port = '/dev/ttyUSB0'
port.dtr = True
port.rts = True
pending = bytearray()
replies = []
process = None
raw_ingress = bytearray()
probe = None


def read_ingress(count):
    incoming = port.read(count)
    raw_ingress.extend(incoming)
    if len(raw_ingress) > 65536:
        del raw_ingress[:-65536]
    return incoming


# Root may finish one admitted 15-second network attempt and five-second
# restoration before returning to management. This is an observation window;
# it changes no provider deadline, admitted work or retry policy.
ROOT_EVENT_SECONDS = 30


def event(name):
    deadline = time.monotonic() + ROOT_EVENT_SECONDS
    while time.monotonic() < deadline:
        pending.extend(read_ingress(min(8192, max(1, port.in_waiting))))
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
    if not line:
        raise RuntimeError('retained owner exited; see native-owner-stderr.log')
    if len(line) > 8192 or not line.endswith('\n'):
        raise RuntimeError('owner frame bound')
    return json.loads(line)


def credited_write(data):
    for offset in range(0, len(data), 64):
        credit = event('native-owner-ingress-credit')
        if credit.get('maximum_bytes') != 64:
            raise RuntimeError('unexpected ingress credit')
        port.write(data[offset:offset + 64])


def carry(frame, wire):
    encoded = json.dumps(frame['request'], separators=(',', ':')).encode()
    signature = bytes(frame['signature'])
    if len(encoded) > 2048 or len(signature) != 64:
        raise RuntimeError('owner command bound')
    port.write(b'owner-control\n')
    ready = event('native-owner-control-ready')
    if ready.get('protocol') != 2 or ready.get('ingress_credit_bytes') != 64:
        raise RuntimeError('owner-control protocol mismatch')
    port.write(len(encoded).to_bytes(2, 'little'))
    credited_write(encoded)
    credited_write(signature)
    if frame['request']['operation'] == 'Prepare':
        event('native-owner-assignment-ready')
        port.write(len(wire).to_bytes(2, 'little'))
        credited_write(wire)
    reply = event('native-owner-control')
    length = reply.get('publication_bytes', 0)
    if not isinstance(length, int) or not 0 <= length <= 4096:
        raise RuntimeError('publication bound')
    deadline = time.monotonic() + 3
    while len(pending) < length and time.monotonic() < deadline:
        pending.extend(read_ingress(length - len(pending)))
    if len(pending) < length:
        raise RuntimeError('publication deadline')
    if length:
        reply['publication'] = list(pending[:length])
        del pending[:length]
    process.stdin.write(json.dumps(reply) + '\n')
    process.stdin.flush()


receipt = {'completed': False, 'proof_class': 'physical-owner-' + args.mode,
           'body_start_attributed': False, 'climate_sampling_claimed': False}
try:
    if args.http_fixture:
        if args.mode != 'run' or args.display:
            raise RuntimeError('HTTP fixture requires the native Face run')
        from native_http_probe import HttpProbe
        probe = HttpProbe(args.capture)
        probe.start()
    if args.display and args.mode == 'start-held':
        raise RuntimeError('display proof supports prepare or finite idle lifecycle')
    wire_name = 'uninstalled-output-assignment-transfer.bin' if args.display else 'uninstalled-assignment-transfer.bin'
    wire = (args.capture / wire_name).read_bytes()
    if not 0 < len(wire) <= 8192:
        raise RuntimeError('transfer bound')
    port.open()
    with (args.capture / 'native-owner-stderr.log').open('w') as errors:
        process = subprocess.Popen([str(args.owner.resolve()), str(args.capture.resolve()),
                                    str(args.admission.resolve()), str(args.keys.resolve())] +
                                   ([] if args.mode == 'prepare' else [args.mode]),
                                   stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=errors, text=True)
        operations = ['Inspect', 'Reserve', 'Prepare']
        if args.mode != 'prepare':
            operations.append('Start')
        for operation in operations:
            frame = host_line()
            assert frame['event'] == 'owner-command' and frame['request']['operation'] == operation
            encoded = json.dumps(frame['request'], separators=(',', ':')).encode()
            signature = bytes(frame['signature'])
            assert len(encoded) <= 2048 and len(signature) == 64
            carry(frame, wire)
        if args.mode == 'run':
            for _ in range(45):
                frame = host_line()
                if frame.get('event') == 'owner-run-complete':
                    receipt = frame
                    break
                if frame.get('event') != 'owner-command':
                    raise RuntimeError('unexpected retained owner event')
                if frame['request']['operation'] not in ['Release', 'Inspect', 'Cancel', 'Retire']:
                    raise RuntimeError('unexpected run operation')
                carry(frame, wire)
            else:
                raise RuntimeError('finite owner run exchange bound')
        else:
            receipt = host_line()
        expected = {'prepare': 'owner-preparation-complete', 'start-held': 'owner-start-held-complete', 'run': 'owner-run-complete'}[args.mode]
        assert receipt['event'] == expected and receipt['completed']
        process.stdin.close()
        if process.wait(timeout=8) != 0:
            raise RuntimeError('retained owner refused preparation')
    port.write(b'offer\n')
    after = event('native-root-offer')
    assert after['appliance_kernel_prepared'] == (args.mode != 'run')
    assert after['assignment_staged'] == args.display
    assert not after['body_installed'] and not after['native_plan_installed']
    (args.capture / 'final-root-offer.json').write_text(json.dumps(after, indent=2) + '\n')
    if probe is not None:
        probe.finish()
        probe = None
    receipt['final_offer_generation'] = after['advertisement']['offer_generation']
    receipt['final_network_configuration'] = after['network_configuration']
    receipt.update(proof_class='physical-owner-' + args.mode,
                   root_heap_used_bytes=after['native_root_heap_used_bytes'],
                   root_heap_peak_bytes=after['native_root_heap_peak_bytes'],
                   native_root_heap_bytes=after['native_root_heap_bytes'])
finally:
    if probe is not None:
        probe.stop()
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
    # These are outputs of the real Rust verifier/owner, not claims inferred
    # from an unsigned control reply or a merely requested operation.
    receipt['body_start_attributed'] = (args.capture / 'body-started-evidence.json').is_file()
    receipt['body_lulled_after_quiescence'] = (args.capture / 'body-lulled-evidence.json').is_file()
    receipt['worker_retirement_acknowledged'] = (args.capture / 'owner-retired.json').is_file()
    receipt['worker_release_acknowledged'] = (args.capture / 'owner-release.json').is_file()
    (args.capture / 'native-owner-uart.bin').write_bytes(raw_ingress)
    receipt['carrier_sha256'] = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
    receipt['owner_executable_sha256'] = hashlib.sha256(args.owner.read_bytes()).hexdigest()
    receipt['retained_uart_bytes'] = len(raw_ingress)
    if process is not None and process.poll() is None:
        process.terminate()
        process.wait(timeout=3)
    (args.capture / 'native-owner-replies.json').write_text(json.dumps(replies, indent=2) + '\n')
    (args.capture / 'native-owner-receipt.json').write_text(json.dumps(receipt, indent=2) + '\n')
    print(json.dumps(receipt))
