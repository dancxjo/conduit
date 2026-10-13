"""Validate actual standalone endpoint STALL, never relabel it data success."""
import hashlib,json,struct
from pathlib import Path
from canonical_decoder import Decoder
base=Path(__file__).resolve().parent
for name,digest in json.loads((base/'sha256.json').read_text()).items():
    assert hashlib.sha256((base/name).read_bytes()).hexdigest()==digest,name
lines=(base/'run/serial.log').read_text().splitlines()
outputs=[i for i,line in enumerate(lines) if line.startswith('CONDUIT_PROTOCOL_OUTPUT port=results ')]
assert len(outputs)==1
decoder=Decoder(bytes.fromhex(lines[outputs[0]+1]));value=decoder.val(decoder.ty())
assert decoder.p==len(decoder.b) and value=={'stalled':b''}
wire=[json.loads(line) for line in (base/'run/wire.jsonl').read_text().splitlines()]
requests=[item for item in wire if item['type']==101 and item['direction']=='qemu-to-device']
replies=[item for item in wire if item['type']==101 and item['direction']=='device-to-qemu']
assert len(requests)==len(replies)==1 and requests[0]['id']==replies[0]['id']
assert struct.unpack('<BBHIH',bytes.fromhex(requests[0]['wire_hex'])[16:])==(129,0,17,0,0)
assert struct.unpack('<BBHIH',bytes.fromhex(replies[0]['wire_hex'])[16:])==(129,4,0,0,0)
allocation=next(line for line in lines if line.startswith('CONDUIT_PROTOCOL_PLAY_ALLOCATIONS '))
stop=next(line for line in lines if line.startswith('CONDUIT_PROTOCOL_PLAY_STOPPED '))
assert 'alloc=0 realloc=0' in allocation
for field in ('body','plan','play'):
    def identity(line):return next(word for word in line.split() if word.startswith(field+'='))
    assert identity(allocation)==identity(stop)
assert outputs[0]<lines.index(allocation)<lines.index(stop) and lines[-1]=='CONDUIT_PROTOCOL_RETIRED complete'
assert not any('REFUSAL' in line for line in lines)
print('standalone-endpoint-read executed=true typed=stalled exact-wire=true zero-allocation=true retired=true')
