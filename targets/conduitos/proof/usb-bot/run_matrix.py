#!/usr/bin/env python3
"""Retain exact native BOT case outcomes; exit nonzero on any proof failure."""
import argparse
import json
import os
from pathlib import Path
import struct
import subprocess
p=argparse.ArgumentParser()
for name in ['xtask','kernel','build-record','package','root-request','output','decoder','expected-block','workspace']:
    p.add_argument('--'+name,required=True)
p.add_argument('--aarch64-firmware')
p.add_argument('--qemu')
p.add_argument('--modes',nargs='+',default=['none','stall-in','disconnect-in','stale-tag','bad-residue','command-failed','phase-error','short-in'])
a=p.parse_args();root=Path(a.output).resolve();root.mkdir(exist_ok=False)
fixture=Path(__file__).resolve().parent
for name in ['xtask','kernel','build_record','package','root_request','decoder','expected_block','workspace']:
    setattr(a,name,str(Path(getattr(a,name)).resolve()))
def command(args,log,env=None):
    with log.open('xb') as stream:
        return subprocess.run(args,stdout=stream,stderr=subprocess.STDOUT,env=env,cwd=a.workspace,check=False).returncode
assert command([a.xtask,'make','conduitos','protocol-image','--kernel',a.kernel,'--build-record',a.build_record,
    '--package',a.package,'--root-request',a.root_request,'--output-dir',str(root/'media')],root/'image.log')==0
results=[]
for mode in a.modes:
    env=dict(os.environ,TMPDIR='/dev/shm',PYTHONDONTWRITEBYTECODE='1',BOT_DEVICE_MODE=mode,
        BOT_DEVICE_OUTPUT=str(root/('device-'+mode)))
    if a.qemu:env['BOT_QEMU']=str(Path(a.qemu).resolve())
    status=command([a.xtask,'make','conduitos','protocol-run','--media',str(root/'media'),
        '--output-dir',str(root/('run-'+mode)),'--seconds','60','--maximum-transcript-bytes','2097152',
        '--emulator',str(fixture/'qemu_bot_wrapper.py'),
        *(['--aarch64-firmware',str(Path(a.aarch64_firmware).resolve())] if a.aarch64_firmware else [])],root/(mode+'.log'),env)
    expected={'stall-in':'stalled','disconnect-in':'provider-lost'}.get(mode,'command-failed')
    decoder_args=([a.decoder,str(root/('run-'+mode)/'serial.log'),'native-storage-reuse',a.expected_block]
        if mode=='none' else [a.decoder,str(root/('run-'+mode)/'serial.log'),'native-storage-terminal',expected])
    if mode in ('hold-in','late-after-cancel') and status==0:
        from verify_cancellation import verify
        try:
            details=verify(root/('run-'+mode),root/('device-'+mode),mode)
            decoded=0
        except (AssertionError, OSError, ValueError) as error:
            details={'error':str(error)}
            decoded=1
        (root/(mode+'-decoded.log')).write_text(json.dumps(details)+'\n')
    else:
        decoded=command(decoder_args,root/(mode+'-decoded.log')) if status==0 else None
    tags=[]
    wire=root/('device-'+mode)/'wire.jsonl'
    if wire.exists():
        for line in wire.read_text().splitlines():
            packet=json.loads(line)
            if packet['direction']=='qemu-to-device' and packet['type']==101:
                body=bytes.fromhex(packet['wire_hex'])[16:];payload=body[10:]
                if (body[0]==2 and len(payload)==31 and payload[:4]==b'USBC'
                    and struct.unpack_from('<I',payload,8)[0]==512
                    and payload[12:16]==bytes([128,0,10,0x28])
                    and payload[17:21]==bytes(4) and payload[22:24]==bytes([0,1])):
                    tag=struct.unpack_from('<I',payload,4)[0]
                    if tag!=999:tags.append(tag)
    expected_tags=list(range(1,17)) if mode=='none' else [1]
    result={'mode':mode,'runner_exit':status,'decoder_exit':decoded,'native_tags':tags,
        'passed':status==0 and decoded==0 and tags==expected_tags}
    results.append(result)
    (root/'results.json').write_text(json.dumps(results,indent=2)+'\n')
    print(json.dumps(result),flush=True)
raise SystemExit(0 if all(r['passed'] for r in results) else 1)
