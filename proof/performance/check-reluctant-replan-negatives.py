#!/usr/bin/env python3
"""Semantic corruption controls after hash verification (rehash-equivalent)."""
import copy, importlib.util, sys
from pathlib import Path
spec=importlib.util.spec_from_file_location('verify',Path(__file__).with_name('verify-reluctant-replan.py'))
module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
rows,status,worker=module.load(sys.argv[1]);module.verify(rows,status,worker)
def row(rows,case): return next(r for r in rows if r['case']==case)
def execution(rows,name): return next(r for r in rows if r['case']=='execution' and r['request']==name)
def decision(rows,label): return next(r for r in rows if r['case']=='decision' and r['label']==label)
controls=[
 ('missing old receipt',lambda r:r.remove(row(r,'initial'))),
 ('missing replacement stage',lambda r:row(r,'definition')['stages'].pop()),
 ('fabricated net benefit',lambda r:decision(r,'persistent')['account']['observations'][0].__setitem__('net_seconds',{'numerator':'999','denominator':'1'})),
 ('forecast terminal substitution',lambda r:decision(r,'persistent')['account']['observations'][0]['candidate'].__setitem__('sign_id','wrong-sign')),
 ('wrong output bytes',lambda r:execution(r,'horizon/1')['receipt']['output']['bytes'].__setitem__(0,0)),
 ('replayed Play',lambda r:execution(r,'horizon/2')['receipt']['active_play'].__setitem__('active_play_id',execution(r,'horizon/1')['receipt']['active_play']['active_play_id'])),
 ('missing admitted horizon request',lambda r:r.remove(execution(r,'horizon/17'))),
 ('noise retriggered',lambda r:decision(r,'noisy')['account'].__setitem__('keep_current',None)),
 ('old receipt custody erased',lambda r:row(r,'summary').__setitem__('old_receipt_after','0'*64)),
 ('foreign stage custodian',lambda r:row(r,'definition')['stages'][0]['source'].__setitem__('boot_id','other-boot')),
 ('fabricated clock interval',lambda r:execution(r,'persistent/0/previous')['service']['range'].__setitem__('upper',{'numerator':'0','denominator':'1'})),
 ('absent queued request',lambda r:r.remove(execution(r,'noisy/1/queued-explicit-request/2'))),
]
for name,mutate in controls:
    changed=copy.deepcopy(rows);mutate(changed)
    try: module.verify(changed,status,worker)
    except (ValueError,KeyError,IndexError,TypeError,AssertionError) as error: print(f'PASS: rejects {name}: {error}')
    else: sys.exit('FAIL: accepted '+name)
