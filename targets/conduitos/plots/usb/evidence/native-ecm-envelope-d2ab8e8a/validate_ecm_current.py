import contextlib,io,runpy,sys,json,struct,hashlib
from pathlib import Path
root=Path(sys.argv[1]) if len(sys.argv)>1 else Path(__file__).resolve().parent; sys.argv=[sys.argv[0],str(root/'serial.log')]
with contextlib.redirect_stdout(io.StringIO()): ns=runpy.run_path(str(Path(__file__).with_name('decode_events.py')))
D=ns['Decoder']; lines=(root/'serial.log').read_text().splitlines(); events=[]
for i,line in enumerate(lines):
 if line.startswith('CONDUIT_PROTOCOL_OUTPUT'):
  d=D(bytes.fromhex(lines[i+1])); v=d.val(d.ty()); assert d.p==len(d.b); events.append(v)
tx=[v['frame']['transmit']['wire'] for v in events if 'transmit' in v.get('frame',{})]
rx=[v['read']['gate']['ready']['deliver-receive']['wire'] for v in events if 'ready' in v.get('read',{}).get('gate',{})]
wire=[json.loads(s) for s in (root/'usb-wire.jsonl').read_text().splitlines()]; out=[]; incoming=[]
for x in wire:
 if x['type']!=101: continue
 b=bytes.fromhex(x['wire_hex']); ep,status,length,stream,high=struct.unpack_from('<BBHIH',b,16); data=b[26:]; assert len(data) in (0,length+(high<<16))
 if x['direction']=='qemu-to-device' and ep==2 and data: out.append(data)
 if x['direction']=='device-to-qemu' and ep==129 and status==0 and data: incoming.append(data)
assert tx==out
assert b''.join(rx)==b''.join(incoming)
assert 1514 in list(map(len,tx)) and 1514 in list(map(len,rx))
assert 'transmitted=1460 received=1460' in '\n'.join(lines)
assert any('PLAY_ALLOCATIONS' in s and 'alloc=0 realloc=0' in s for s in lines)
assert any('PLAY_STOPPED' in s for s in lines) and 'CONDUIT_PROTOCOL_RETIRED complete' in lines
summary={'tier':'native-x86_64-qemu-external-usb-device','tx_frames':len(tx),'rx_frames':len(rx),'tx_lengths':list(map(len,tx)),'rx_lengths':list(map(len,rx)),'wire_exact':True,'tcp_bytes_each_direction':1460,'play_allocations':0,'play_reallocations':0,'acknowledged_stop':True,'normal_retirement':True,'physical_proof':False}
(root/'validation.json').write_text(json.dumps(summary,indent=2)+'\n');print(json.dumps(summary))
