#!/usr/bin/env python3
"""Read-only physical Root software availability; no Body or native IO starts."""
import argparse
import json
from pathlib import Path
import time
import serial

parser = argparse.ArgumentParser()
parser.add_argument('--port', default='/dev/ttyUSB0')
parser.add_argument('--output', required=True)
parser.add_argument('--profile', choices=('native-root', 'native-face-root'), default='native-root')
args = parser.parse_args()
out = Path(args.output)
out.mkdir(parents=True, exist_ok=False)
port = serial.Serial(port=None, baudrate=115200, timeout=.1)
port.port = args.port
port.dtr = True
port.rts = True
pending = bytearray()
raw = bytearray()
replies = []

def request(command, event, timeout):
    port.write(command + b'\n')
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        pending.extend(port.read(8192))
        assert len(pending) <= 65536, 'reply exceeded finite wire profile'
        while b'\n' in pending:
            line, _, tail = pending.partition(b'\n')
            pending[:] = tail
            raw.extend(line + b'\n')
            try:
                value = json.loads(line)
            except ValueError:
                continue
            if isinstance(value, dict) and value.get('event') == event:
                replies.append(value)
                return value
    raise RuntimeError('no complete ' + event + ' before deadline')

result = {'proof_class': 'physical-initialized-software-readiness', 'completed': False,
          'body_start_claimed': False, 'climate_sampling_claimed': False}
try:
    port.open()
    before = request(b'offer', 'native-root-offer', 8)
    software = request(b'software-offer', 'native-software-offer', 14)
    policy = request(b'appliance-policy', 'native-appliance-policy', 8)
    after = request(b'offer', 'native-root-offer', 8)
    for field in ['host_id', 'boot_id', 'offer_generation']:
        assert before['advertisement'][field] == software[field] == policy[field] == after['advertisement'][field]
    assert not software['body_installed'] and not software['native_plan_installed']
    assert (software['count'], software['period_ms']) == (960, 60000)
    assert len(software['source_identity']) == 32
    assert policy['source_identity'] == software['source_identity']
    assert (policy['count'], policy['period_ms']) == (software['count'], software['period_ms'])
    assert not policy['body_installed'] and not policy['native_plan_installed'] and not policy['capability_possession_issued']
    assert len(policy['authority_grants']) == 2
    caps = software['capabilities']
    assert len(caps) == 13
    assert len({c['capability_id'] for c in caps}) == 13
    assert not {'time/tick', 'machine/pulse/read'} & {c['kind_id'] for c in caps}
    for cap in caps:
        assert cap['artifact_id'] == 'conduit-environment-runtime/appliance-software@1'
        assert cap['execution_profile_id'] == 'conduit.environment/prepared-software@1'
        assert not cap['resource_requirements'] and not cap['authority_requirements']
    encoded = json.dumps(caps, ensure_ascii=False, separators=(',', ':')).encode()
    assert len(encoded) == software['capabilities_bytes']
    assert before['native_root_heap_used_bytes'] == after['native_root_heap_used_bytes']
    # Readiness inspection must not dispatch IO; earlier completed network
    # control is allowed so this also checks retained software after fallback.
    assert before['network_control_dispatches'] == after['network_control_dispatches']
    expected_root_bytes = 112 * 1024 + (4080 if args.profile == 'native-face-root' else 0)
    assert before['native_root_heap_bytes'] == after['native_root_heap_bytes'] == expected_root_bytes
    assert before['native_sdk_heap_bytes'] == after['native_sdk_heap_bytes'] == 72 * 1024
    result.update(completed=True, profile=args.profile, expected_root_heap_bytes=expected_root_bytes,
                  boot_id=software['boot_id'], capabilities_bytes=len(encoded),
                  root_heap_used_bytes=after['native_root_heap_used_bytes'],
                  root_heap_peak_bytes=after['native_root_heap_peak_bytes'],
                  sdk_heap_peak_bytes=after['native_sdk_heap_peak_bytes'])
    print(json.dumps(result), flush=True)
finally:
    port.close()
    (out / 'result.json').write_text(json.dumps(result, indent=2) + '\n')
    (out / 'replies.json').write_text(json.dumps(replies, indent=2) + '\n')
    (out / 'serial.log').write_bytes(raw)
