"""Check retained canonical control result against the external device wire."""
import hashlib
import json
from pathlib import Path
from canonical_decoder import Decoder
base = Path(__file__).resolve().parent
digests = json.loads((base / 'sha256.json').read_text())
for relative, digest in digests.items():
    assert hashlib.sha256((base / relative).read_bytes()).hexdigest() == digest, relative
image = json.loads((base / 'image-receipt.json').read_text())
root = json.loads((base / 'root.json').read_text())
assert hashlib.sha256((base / 'package.json').read_bytes()).hexdigest() == root['package_sha256']
lines = (base / 'run/serial.log').read_text().splitlines()
indices = [i for i, line in enumerate(lines) if line.startswith('CONDUIT_PROTOCOL_OUTPUT port=results ')]
assert len(indices) == 1
index = indices[0]
decoder = Decoder(bytes.fromhex(lines[index + 1]))
result = decoder.val(decoder.ty())
assert decoder.p == len(decoder.b)
completed = result['completed']
descriptor = bytes(octet[0] for octet in completed['input'])
expected = bytes.fromhex('1201000202000008ffff17f1000101020301')
assert descriptor == expected
assert completed['transferred'] == bytes([18, 0]) and completed['short'] == bytes([0])
wire = [json.loads(line) for line in (base / 'run/wire.jsonl').read_text().splitlines()]
assert any(item['direction'] == 'device-to-qemu' and bytes.fromhex(item['wire_hex']).endswith(expected) for item in wire)
allocation = next(line for line in lines if line.startswith('CONDUIT_PROTOCOL_PLAY_ALLOCATIONS '))
stop = next(line for line in lines if line.startswith('CONDUIT_PROTOCOL_PLAY_STOPPED '))
assert 'alloc=0 realloc=0' in allocation
assert index < lines.index(allocation) < lines.index(stop) < len(lines) - 1
for field in ('body', 'plan', 'play'):
    def selected(line):
        return next(word for word in line.split() if word.startswith(field + '='))
    assert selected(allocation) == selected(stop)
assert lines[-1] == 'CONDUIT_PROTOCOL_RETIRED complete'
assert not any('REFUSAL' in line for line in lines)
print('canonical descriptor=18 exact-wire=true zero-play-allocation=true matching-stop=true')
