#!/usr/bin/env python3
"""Attended UART proof through the ordinary host Body admission verifier."""
import argparse
import json
from pathlib import Path
import selectors
import subprocess
import time
import serial

parser = argparse.ArgumentParser()
parser.add_argument('--verifier', type=Path, required=True)
parser.add_argument('--output', type=Path, required=True)
parser.add_argument('--port', default='/dev/ttyUSB0')
args = parser.parse_args()
args.output.mkdir(parents=True, exist_ok=False)
port = serial.Serial(port=None, baudrate=115200, timeout=.1, write_timeout=2)
port.port = args.port
port.dtr = True
port.rts = True
pending = bytearray()
observations = []


def receive(predicate, seconds=8, readiness_ping=None):
    deadline = time.monotonic() + seconds
    next_ping = time.monotonic()
    while time.monotonic() < deadline:
        if readiness_ping is not None and time.monotonic() >= next_ping:
            port.write(readiness_ping)
            next_ping = time.monotonic() + 2
        pending.extend(port.read(8192))
        if len(pending) > 16384:
            raise RuntimeError('UART ingress capacity exceeded')
        while b'\n' in pending:
            line, _, tail = pending.partition(b'\n')
            pending[:] = tail
            if len(line) > 8192:
                raise RuntimeError('UART frame capacity exceeded')
            try:
                value = json.loads(line)
            except ValueError:
                continue
            observations.append(value)
            if predicate(value):
                return value
    raise RuntimeError('UART response deadline exceeded')


def host_line(process):
    with selectors.DefaultSelector() as selector:
        selector.register(process.stdout, selectors.EVENT_READ)
        if not selector.select(8):
            raise RuntimeError('host verifier deadline exceeded')
        line = process.stdout.readline(65537)
        if len(line) > 65536 or not line.endswith('\n'):
            raise RuntimeError('host verifier incomplete/oversized frame')
        return json.loads(line)


process = None
try:
    port.open()
    # Boot may still be compiling retained Source while early UART input is lost.
    # Only read-only readiness is polled; challenges and mutations are not retried.
    receive(lambda v: v.get('event') == 'native-root-offer', seconds=25,
            readiness_ping=b'offer\n')
    port.write(b'CONDUIT_BODY_ADVERTISE_V1\n')
    offered = receive(lambda v: v.get('protocol') == 1 and 'verifying_key' in v)
    if offered['advertisement']['host_id'] != 'esp32/b0a732286178':
        raise RuntimeError('unexpected physical ESP32')
    with (args.output / 'host-stderr.log').open('w') as errors:
        process = subprocess.Popen([str(args.verifier.resolve())], stdin=subprocess.PIPE,
                                   stdout=subprocess.PIPE, stderr=errors, text=True)
        process.stdin.write(json.dumps(offered) + '\n')
        process.stdin.flush()
        challenge = host_line(process)
        stale = dict(challenge)
        stale['boot_id'] += '/stale'
        port.write(json.dumps(stale, separators=(',', ':')).encode() + b'\n')
        refused = receive(lambda v: v.get('event') == 'native-root-request-refused')
        assert not refused['native_plan_installed']
        port.write(b'CONDUIT_BODY_ADVERTISE_V1\n')
        current = receive(lambda v: v.get('protocol') == 1 and 'verifying_key' in v)
        assert current['advertisement'] == offered['advertisement']
        assert current['verifying_key'] == offered['verifying_key']
        assert current['freshness_sequence'] > offered['freshness_sequence']
        port.write(json.dumps(challenge, separators=(',', ':')).encode() + b'\n')
        proof = receive(lambda v: v.get('admission_id') == challenge['admission_id'])
        process.stdin.write(json.dumps(proof) + '\n')
        process.stdin.flush()
        result = host_line(process)
        process.stdin.close()
        if process.wait(timeout=8) != 0:
            raise RuntimeError('ordinary Body admission verifier failed')
    result['physical_stale_boot_refused'] = True
    assert result['proof_class'] == 'physical-ambient-admission'
    assert result['tampering_refused'] and result['replay_refused']
    assert not result['play_started'] and not result['body_installed_on_mcu']
    port.write(b'offer\n')
    after = receive(lambda v: v.get('event') == 'native-root-offer')
    assert not after['body_installed'] and not after['native_plan_installed']
    assert after['advertisement'] == offered['advertisement']
    (args.output / 'admission.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps({'completed': True, 'proof_class': result['proof_class'],
                      'host': result['credential']['host_id'],
                      'boot': result['credential']['boot_id'],
                      'body_installed_on_mcu': False, 'play_started': False}))
finally:
    port.close()
    (args.output / 'observations.json').write_text(json.dumps(observations, indent=2) + '\n')
    if process is not None and process.poll() is None:
        process.terminate()
        process.wait(timeout=5)
