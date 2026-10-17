"""Revalidate corrected-package native ECM detach without claiming success."""
import contextlib, io, json, runpy, sys
from pathlib import Path
root = Path(__file__).resolve().parent
serial = (root/'serial.log').read_text()
packets = [json.loads(s) for s in (root/'usb-wire.jsonl').read_text().splitlines()]
detach = next(i for i,p in enumerate(packets) if p['direction']=='device-to-qemu' and p['type']==2)
assert not any(p['direction']=='qemu-to-device' and p['type'] in (100,101) for p in packets[detach+1:])
assert json.loads((root/'receipt.json').read_text())['outcome']=='play-refused:protocol-network-not-complete'
sys.argv = ['decoder', str(root/'serial.log')]
with contextlib.redirect_stdout(io.StringIO()) as decoded:
    runpy.run_path(str(root.parent/'decode_events.py'))
assert decoded.getvalue().strip().splitlines()[-1]=="{'provider-lost': 0}"
assert "'gate': {'provider-lost': 0}" in decoded.getvalue()
lines = serial.splitlines()
alloc = next(s for s in lines if s.startswith('CONDUIT_PROTOCOL_PLAY_ALLOCATIONS '))
stop = next(s for s in lines if s.startswith('CONDUIT_PROTOCOL_PLAY_STOPPED '))
assert 'alloc=0 realloc=0' in alloc
fields = lambda line: dict(item.split('=',1) for item in line.split()[1:] if '=' in item)
assert all(fields(alloc)[k]==fields(stop)[k] for k in ['body','plan','play'])
assert 'CONDUIT_PROTOCOL_RETIRED complete' not in serial
print(json.dumps({'class_outcome':'provider-lost','root_outcome':'protocol-network-not-complete','effects_after_detach':0,'play_allocations':0,'play_reallocations':0,'acknowledged_stop':True,'normal_retirement':False,'physical_proof':False}))
