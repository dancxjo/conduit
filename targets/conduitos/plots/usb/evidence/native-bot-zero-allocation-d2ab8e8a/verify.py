"""Independent exact block/output and native-wire verification."""
import contextlib, io, json, runpy, struct, sys
from pathlib import Path
root=Path(__file__).resolve().parent
lines=(root/'serial.log').read_text().splitlines()
sys.argv=['decoder',str(root/'serial.log')]
with contextlib.redirect_stdout(io.StringIO()):
    decoder=runpy.run_path(str(root.parent/'native-ecm-envelope-d2ab8e8a/decode_events.py'))['Decoder']
expected=(root.parent/'native-bot-e9a277e4/expected-block.bin').read_bytes()
assert len(expected)==512
outputs=[]
for i,line in enumerate(lines):
    if not line.startswith('CONDUIT_PROTOCOL_OUTPUT port=result '):continue
    d=decoder(bytes.fromhex(lines[i+1])); value=d.val(d.ty()); assert d.p==len(d.b)
    assert set(value)=={'completed'}
    result=value['completed']; ordinal=len(outputs)
    for name,n in [('actual',512),('block_size',512),('count',1),('lba',0),('ordinal',ordinal),('residue',0),('short',0)]:
        assert int.from_bytes(result[name],'little')==n,(ordinal,name)
    assert result['bytes']==expected
    outputs.append(result)
assert len(outputs)==16
packets=[json.loads(s) for s in (root/'wire.jsonl').read_text().splitlines()]
tags=[]; blocks=[]
for p in packets:
    if p['type']!=101:continue
    body=bytes.fromhex(p['wire_hex'])[16:];ep,status,length,stream,high=struct.unpack_from('<BBHIH',body);payload=body[10:]
    if p['direction']=='qemu-to-device' and ep==2 and len(payload)==31 and payload[:4]==b'USBC':
        tag=struct.unpack_from('<I',payload,4)[0]
        if 1<=tag<=16:
            assert payload[12:16]==bytes([128,0,10,0x28])
            assert struct.unpack_from('<I',payload,8)[0]==512
            tags.append(tag)
    if p['direction']=='device-to-qemu' and ep==129 and status==0 and length==512:
        assert payload==expected;blocks.append(payload)
assert tags==list(range(1,17)) and len(blocks)==16
alloc=next(s for s in lines if s.startswith('CONDUIT_PROTOCOL_PLAY_ALLOCATIONS '))
stop=next(s for s in lines if s.startswith('CONDUIT_PROTOCOL_PLAY_STOPPED '))
fields=lambda s:dict(v.split('=',1) for v in s.split()[1:] if '=' in v)
assert fields(alloc)['alloc']==fields(alloc)['realloc']=='0'
assert all(fields(alloc)[k]==fields(stop)[k] for k in ['body','plan','play'])
assert json.loads((root/'receipt.json').read_text())['outcome']=='play-retired'
assert 'CONDUIT_PROTOCOL_RETIRED complete' in lines
print(json.dumps({'proof_class':'native-x86_64-emulator-external-device','block_outputs':16,'wire_command_tags':tags,'exact_512_byte_blocks':16,'play_allocations':0,'play_reallocations':0,'acknowledged_stop':True,'normal_retirement':True,'physical_proof':False,'initial_decoder_incompatible':True}))
