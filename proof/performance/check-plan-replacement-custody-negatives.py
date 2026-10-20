#!/usr/bin/env python3
"""Corrupt actual retained lifecycle records and require independent refusal."""
import copy
import gzip
import json
from pathlib import Path
import runpy
import sys

verify = runpy.run_path(str(Path(__file__).with_name('verify-plan-replacement-custody.py')))['verify']
path = Path(sys.argv[1])
raw = gzip.decompress(path.read_bytes()) if path.suffix == '.gz' else path.read_bytes()
log = raw.decode()
records = verify(log)
unknown = 'uncertain-old-Fore-new-Plan-and-Play'
cancelled = 'accepted-output-drained-before-cancelled-replacement'
lost = 'selected-provider-loss-independent-live-Play'
controls = [
    ('payload fingerprint', unknown, lambda r: r['old_pending_output'].__setitem__('sha256', '0' * 64)),
    ('erased external uncertainty', unknown, lambda r: r['old_pending_output'].__setitem__('stage', 'adapter-accepted')),
    ('wrong cache/executed Plan binding', unknown, lambda r: r.__setitem__('cache_replacement_result', r['cache_old_result'])),
    ('replayed old command', cancelled, lambda r: r['replacement_items'].append('Buy milk')),
    ('delivery after cancellation', cancelled, lambda r: r.__setitem__('cancelled_event', r['delivered_event'])),
    ('conflated independent Play', lost, lambda r: r.__setitem__('unaffected_play', r['old_play'])),
]
for name, case, mutate in controls:
    changed = copy.deepcopy(next(r for r in records if r['case'] == case))
    mutate(changed)
    lines = []
    for line in log.splitlines():
        if line.startswith('{') and json.loads(line).get('case') == case:
            line = json.dumps(changed, separators=(',', ':'))
        lines.append(line)
    try:
        verify('\n'.join(lines))
    except (ValueError, KeyError, TypeError, IndexError) as error:
        print(f'PASS: rejects {name}: {error}')
    else:
        sys.exit(f'FAIL: accepted {name}')
