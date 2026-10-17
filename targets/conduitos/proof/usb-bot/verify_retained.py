#!/usr/bin/env python3
"""Verify retained historical BOT receipts; never claim fresh runtime execution."""
import argparse
import gzip
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import tempfile
from verify_cancellation import verify
p=argparse.ArgumentParser()
p.add_argument('--evidence',required=True)
p.add_argument('--decoder',required=True)
a=p.parse_args();base=Path(a.evidence).resolve();decoder=Path(a.decoder).resolve()
index=json.loads((base/'index.json').read_text())
package=(base/'protocol-source.json').read_bytes()
request=json.loads((base/'root-request.json').read_text())
assert request['package_sha256']==hashlib.sha256(package).hexdigest()
for arch in ('x86_64','aarch64'):
    manifest=json.loads((base/(arch+'-build-manifest.json')).read_text())
    assert manifest['source_identity']==index['runtime_source_commit']
assert len(index['cases'])==20
seen=set()
for case in index['cases']:
    key=(case['architecture'],case['mode']);assert key not in seen;seen.add(key)
    mode=case['mode']
    with tempfile.TemporaryDirectory(prefix='bot-receipt-') as temporary:
        work=Path(temporary)
        for name,entry in case['files'].items():
            path=(base/entry['path']).resolve();assert path.is_relative_to(base)
            data=gzip.decompress(path.read_bytes())
            assert hashlib.sha256(data).hexdigest()==entry['sha256']
            (work/name).write_bytes(data)
        if mode in ('hold-in','late-after-cancel'):
            verify(work,work,mode)
        else:
            tag={'stall-in':'stalled','disconnect-in':'provider-lost'}.get(mode,'command-failed')
            arguments=([str(decoder),str(work/'serial.log'),'native-storage-reuse',str(base/'expected-block.bin')]
                if mode=='none' else [str(decoder),str(work/'serial.log'),'native-storage-terminal',tag])
            subprocess.run(arguments,check=True,stdout=subprocess.DEVNULL)
        packets=[json.loads(line) for line in (work/'wire.jsonl').read_text().splitlines()]
        configs=[i for i,packet in enumerate(packets) if packet['direction']=='qemu-to-device'
            and packet['type']==6 and bytes.fromhex(packet['wire_hex'])[16:]==bytes([1])]
        assert configs
        commands=[]
        for packet in packets[configs[-1]+1:]:
            if packet['direction']!='qemu-to-device' or packet['type']!=101:continue
            raw=bytes.fromhex(packet['wire_hex']);body=raw[16:]
            endpoint,status,length,stream,high=struct.unpack_from('<BBHIH',body)
            if endpoint!=2:continue
            assert status==stream==high==0 and length==31 and len(body)==41
            cbw=body[10:];tag=struct.unpack_from('<I',cbw,4)[0]
            expected=b'USBC'+struct.pack('<II',tag,512)+bytes([128,0,10,40,0])+bytes(4)+bytes([0,0,1,0])+bytes(6)
            assert cbw==expected
            commands.append(tag)
        assert commands==(list(range(1,17)) if mode=='none' else [1]),(key,commands)
    print(json.dumps({'architecture':key[0],'mode':mode,'verified':True}),flush=True)
print(json.dumps({'retained_cases':20,'runtime_source_commit':index['runtime_source_commit'],'fresh_execution':False}))
