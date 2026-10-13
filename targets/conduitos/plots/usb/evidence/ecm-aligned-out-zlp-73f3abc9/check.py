"""Verify actual complete ECM transfer boundaries and Root-admitted TCP bytes."""
import hashlib,json,struct
from pathlib import Path
from canonical_decoder import Decoder
base=Path(__file__).resolve().parent
for name,digest in json.loads((base/'sha256.json').read_text()).items():
    assert hashlib.sha256((base/name).read_bytes()).hexdigest()==digest,name
lines=(base/'run/serial.log').read_text().splitlines(); reads=[]; writes=[]
for index,line in enumerate(lines):
    if line.startswith('CONDUIT_PROTOCOL_OUTPUT port=observed '):
        decoder=Decoder(bytes.fromhex(lines[index+1])); value=decoder.val(decoder.ty())
        assert decoder.p==len(decoder.b)
        if 'read' in value:reads.append(value['read'])
        if 'write' in value:writes.append(int.from_bytes(value['write']['completed']['actual'],'little'))
assert writes==[42,66,54,512,0,54]
assert [int.from_bytes(value['ordinal'],'little') for value in reads]==list(range(5))
packets=[json.loads(line) for line in (base/'usb-wire.jsonl').read_text().splitlines()]
transfers=[]
for index,item in enumerate(packets):
    if item['type']==101 and item['direction']=='qemu-to-device':
        body=bytes.fromhex(item['wire_hex'])[16:]
        ep,status,length,stream,high=struct.unpack_from('<BBHIH',body)
        if ep==2:transfers.append((index,item['id'],length,body[10:]))
assert [length for _,_,length,_ in transfers]==writes
frame_index,frame_id,_,frame=transfers[3]
zlp_index,zlp_id,zlp_length,zlp=transfers[4]
assert frame_id!=zlp_id and zlp_length==0 and zlp==b''
peer=json.loads((base/'peer-receipt.json').read_text())
assert peer['outcome']=='exact-request-received-response-sent-client-eof-observed'
request=bytes.fromhex(peer['request_hex']); response=bytes.fromhex(peer['response_hex'])
assert request==bytes(index%256 for index in range(458))
assert response==bytes((255-index)%256 for index in range(74))
assert len(frame)==512 and frame[12:14]==bytes.fromhex('0800') and frame[14] & 15==5 and frame[46]>>4==5
assert frame[54:]==request
received=reads[3]['gate']['ready']['deliver-receive']['wire']
assert len(received)==128 and received[54:]==response and reads[3]['short']==bytes([1])
assert not any(int.from_bytes(value['actual'],'little')==0 for value in reads)
allocation=next(line for line in lines if line.startswith('CONDUIT_PROTOCOL_PLAY_ALLOCATIONS '))
stop=next(line for line in lines if line.startswith('CONDUIT_PROTOCOL_PLAY_STOPPED '))
assert 'alloc=0 realloc=0' in allocation
for field in ('body','plan','play'):
    def identity(line):return next(word for word in line.split() if word.startswith(field+'='))
    assert identity(allocation)==identity(stop)
assert lines.index(allocation)<lines.index(stop) and lines[-1]=='CONDUIT_PROTOCOL_RETIRED complete'
assert not any('REFUSAL' in line for line in lines)
print('TX=512+distinct-ZLP RX=128 TCP=458/74 typed-boundaries=true zero-allocation=true retired=true')
