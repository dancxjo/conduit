"""Check a composed guest recovery trace; no standalone fixture substitute."""
import argparse,json,struct
from pathlib import Path
from canonical_decoder import Decoder
p=argparse.ArgumentParser()
p.add_argument('--serial',required=True)
p.add_argument('--wire',required=True)
p.add_argument('--phase-error',action='store_true')
a=p.parse_args()
lines=Path(a.serial).read_text().splitlines()
values=[]
for index,line in enumerate(lines):
    if line.startswith('CONDUIT_PROTOCOL_OUTPUT port=result '):
        decoder=Decoder(bytes.fromhex(lines[index+1]))
        value=decoder.val(decoder.ty())
        assert decoder.p==len(decoder.b)
        values.append(value)
first_result = 'command-failed' if a.phase_error else 'stalled'
assert len(values)==2 and list(values[0])==[first_result] and list(values[1])==['completed']
for value in values:
    identity=next(iter(value.values()))
    for key,expected in [('block_size',512),('count',1),('lba',0),('ordinal',0)]:
        assert int.from_bytes(identity[key],'little')==expected
completed=values[1]['completed']
expected_block=bytes((index*37+11)%256 for index in range(512))
assert completed['bytes']==expected_block
assert int.from_bytes(completed['actual'],'little')==512
assert int.from_bytes(completed['residue'],'little')==0 and completed['short']==bytes([0])
packets=[json.loads(line) for line in Path(a.wire).read_text().splitlines()]
cbws=[]; recovery=[]; stalled=[]; data=[]; statuses=[]; phase_errors=[]
for index,item in enumerate(packets):
    body=bytes.fromhex(item['wire_hex'])[16:]
    if item['type']==101:
        ep,status,length,stream,high=struct.unpack_from('<BBHIH',body)
        payload=body[10:]
        if item['direction']=='qemu-to-device' and ep==2:
            assert len(payload)==31 and payload[:4]==b'USBC'
            if (struct.unpack_from('<I',payload,8)[0]==512
                and payload[12:16]==bytes([128,0,10,0x28])
                and payload[17:21]==bytes(4) and payload[22:24]==bytes([0,1])):
                cbws.append((index,struct.unpack_from('<I',payload,4)[0]))
        if item['direction']=='device-to-qemu' and ep==129 and cbws:
            if status==4:
                assert length==0 and not payload
                stalled.append(index)
            elif length==512:
                assert status==0
                expected_data = bytes((i*19+5)%256 for i in range(512)) if a.phase_error and len(cbws)==1 else expected_block
                assert payload==expected_data
                data.append(index)
            elif length==13:
                assert status==0
                if a.phase_error and len(cbws)==1:
                    assert payload==b'USBS'+struct.pack('<IIB',1,0xdeadbeef,2)
                    phase_errors.append(index)
                else:
                    assert payload==b'USBS'+struct.pack('<IIB',2,0,0)
                    statuses.append(index)
    if item['type']==100 and item['direction']=='device-to-qemu' and cbws:
        ep,request,kind,status,value,target,length=struct.unpack_from('<BBBBHHH',body)
        if (kind,request) in [(0x21,0xff),(0x02,1)]:
            assert (ep,status,value,length)==(0,0,0,0) and len(body)==10
            matching=[candidate for candidate in packets[:index]
                if candidate['type']==100 and candidate['direction']=='qemu-to-device'
                and candidate['id']==item['id']]
            assert len(matching)==1
            assert bytes.fromhex(matching[0]['wire_hex'])[16:]==struct.pack('<BBBBHHH',ep,request,kind,0,value,target,0)
            recovery.append((index,kind,request,target))
assert [tag for _,tag in cbws]==[1,2]
assert len(statuses)==1
if a.phase_error:
    assert not stalled and len(data)==2 and len(phase_errors)==1
    assert cbws[0][0]<data[0]<phase_errors[0]
    fault=phase_errors[0]
else:
    assert len(stalled)==len(data)==1 and not phase_errors
    fault=stalled[0]
assert [(kind,request,target) for _,kind,request,target in recovery]==[(0x21,0xff,0),(0x02,1,0x81),(0x02,1,2)]
assert cbws[0][0]<fault<recovery[0][0]<recovery[1][0]<recovery[2][0]<cbws[1][0]<data[-1]<statuses[0]
assert not any(item['type']==101 for item in packets[fault+1:cbws[1][0]])
assert not any(item['type']==2 and item['direction']=='device-to-qemu' for item in packets[cbws[0][0]:statuses[0]+1])
play=next(line for line in lines if line.startswith('CONDUIT_PROTOCOL_PLAY '))
allocation=next(line for line in lines if line.startswith('CONDUIT_PROTOCOL_PLAY_ALLOCATIONS '))
stop=next(line for line in lines if line.startswith('CONDUIT_PROTOCOL_PLAY_STOPPED '))
assert 'alloc=0 realloc=0' in allocation
for field in ('body','plan','play'):
    def identity(line):return next(word for word in line.split() if word.startswith(field+'='))
    assert identity(play)==identity(allocation)==identity(stop)
assert lines.index(play)<lines.index(allocation)<lines.index(stop) and lines[-1]=='CONDUIT_PROTOCOL_RETIRED complete'
assert not any('REFUSAL' in line for line in lines)
print(f'results={first_result},completed semantic-ordinals=0,0 tags=1,2 reset-acks=3 exact-block=true zero-allocation=true retired=true')
