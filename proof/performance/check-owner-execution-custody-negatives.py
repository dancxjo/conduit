#!/usr/bin/env python3
"""Reject corrupted actual owner records, including a rehashed Play substitution."""
import copy
import gzip
import hashlib
import json
from pathlib import Path
import runpy
import sys

verify = runpy.run_path(str(Path(__file__).with_name('verify-owner-execution-custody.py')))['verify']
p = Path(sys.argv[1])
raw = gzip.decompress(p.read_bytes()) if p.suffix == '.gz' else p.read_bytes()
log = raw.decode()
records = verify(log)
replacement = 'durable-normal-owner-replacement-history'
retry = 'owner-terminal-publication-retry-without-replay'
capacity = 'normal-owner-custody-capacity-before-effects'

def forget_old(row):
    i = row['history']['receipts'].index(row['first_receipt'])
    del row['history']['receipts'][i]
    del row['history']['receipt_digests'][i]

def rebind_play(row):
    before = copy.deepcopy(row['latest_receipt'])
    row['latest_receipt']['read_play']['active_play_id'] = row['first_receipt']['read_play']['active_play_id']
    h = row['history']
    i = h['receipts'].index(before)
    h['receipts'][i] = copy.deepcopy(row['latest_receipt'])
    record = {'schema':'conduit.body/execution-custody@1','host_id':h['custodian_host_id'],'body_id':h['body_id'],'receipt':h['receipts'][i]}
    h['receipt_digests'][i] = 'sha256:' + hashlib.sha256(json.dumps(record,separators=(',',':'),ensure_ascii=False).encode()).hexdigest()

controls = [
    ('erased old receipt', replacement, forget_old),
    ('changed receipt fingerprint', replacement, lambda r:r['history']['receipt_digests'].__setitem__(0, '0'*64)),
    ('unavailable historical prefix', replacement, lambda r:r['history'].__setitem__('legacy_prefix_unavailable',True)),
    ('rehashed old Play substitution', replacement, rebind_play),
    ('duplicated terminal after retry', retry, lambda r:(r['history']['receipts'].append(copy.deepcopy(r['receipt'])),r['history']['receipt_digests'].append(r['history']['receipt_digests'][0]))),
    ('effects after capacity refusal', capacity, lambda r:r.__setitem__('host_started',True)),
]
for name,case,mutate in controls:
    changed = copy.deepcopy(next(r for r in records if r['case']==case))
    mutate(changed)
    lines=[]
    for line in log.splitlines():
        start = line.find('{')
        if start >= 0:
            try:
                row, end = json.JSONDecoder().raw_decode(line[start:])
            except ValueError:
                row, end = {}, 0
            if isinstance(row, dict) and row.get('case') == case:
                line = line[:start] + json.dumps(changed,separators=(',',':'),ensure_ascii=False) + line[start+end:]
        lines.append(line)
    try:
        verify('\n'.join(lines))
    except (ValueError,KeyError,IndexError,TypeError) as error:
        print(f'PASS: rejects {name}: {error}')
    else:
        sys.exit(f'FAIL: accepted {name}')
