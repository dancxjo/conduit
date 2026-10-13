"""Check actual aligned ECM frames, separate ordinals and terminal identity."""
import hashlib
import json
from pathlib import Path
from canonical_decoder import Decoder
base = Path(__file__).resolve().parent
for name, digest in json.loads((base/'sha256.json').read_text()).items():
    assert hashlib.sha256((base/name).read_bytes()).hexdigest() == digest, name
lines = (base/'run/serial.log').read_text().splitlines()
reads = []
for index, line in enumerate(lines):
    if line.startswith('CONDUIT_PROTOCOL_OUTPUT port=observed '):
        decoder = Decoder(bytes.fromhex(lines[index+1]))
        value = decoder.val(decoder.ty())
        assert decoder.p == len(decoder.b)
        if 'read' in value:
            reads.append(value['read'])
assert [int.from_bytes(value['ordinal'], 'little') for value in reads] == list(range(8))
wire = [json.loads(line) for line in (base/'usb-wire.jsonl').read_text().splitlines()]
for ordinal, length, marker in [(1,128,0x31),(2,512,0x72)]:
    expected = bytes.fromhex('ffffffffffff02000000000288b5') + bytes([marker])*(length-14)
    result = reads[ordinal]
    assert result['gate']['ready']['deliver-receive']['wire'] == expected
    assert int.from_bytes(result['actual'], 'little') == length and result['short'] == bytes([1])
    assert any(item['direction']=='device-to-qemu' and bytes.fromhex(item['wire_hex']).endswith(expected) for item in wire)
assert any('transmitted=1460 received=1460' in line for line in lines)
allocation = next(line for line in lines if line.startswith('CONDUIT_PROTOCOL_PLAY_ALLOCATIONS '))
stop = next(line for line in lines if line.startswith('CONDUIT_PROTOCOL_PLAY_STOPPED '))
assert 'alloc=0 realloc=0' in allocation
for field in ('body','plan','play'):
    def identity(line):
        return next(word for word in line.split() if word.startswith(field+'='))
    assert identity(allocation) == identity(stop)
assert lines.index(allocation) < lines.index(stop) and lines[-1]=='CONDUIT_PROTOCOL_RETIRED complete'
assert not any('REFUSAL' in line for line in lines)
print('frames=128,512 ordinals=1,2 exact-wire=true TCP=1460/1460 zero-allocation=true retired=true')
