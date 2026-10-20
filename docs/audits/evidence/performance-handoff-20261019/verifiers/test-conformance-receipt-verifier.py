#!/usr/bin/env python3
"""Positive and forged-receipt controls; no timing acceptance implied."""
import copy, hashlib, importlib.util, json, tempfile
from pathlib import Path

HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[1]
spec=importlib.util.spec_from_file_location('public_verify',HERE/'verify-compact-local-public.py')
v=importlib.util.module_from_spec(spec);spec.loader.exec_module(v)
public=ROOT/'work/compact-local-public-capture-1'
worker=ROOT/'work/frozen-compact-local-36c6ff524/fargan_performance'
receipt=json.loads((public/'status.json').read_text())
source=receipt['git_sha']
mutants=[]
with tempfile.TemporaryDirectory(dir=ROOT/'work') as tmp:
    root=Path(tmp)
    originals={
        'conformance-reference-status.json':(public/'status.json').read_bytes(),
        'conformance-reference-metadata.json':(public/'metadata.json').read_bytes(),
        'conformance.stdout.log':(public/'conformance.stdout.log').read_bytes(),
        'conformance.stderr.log':(public/'conformance.stderr.log').read_bytes(),
    }
    def reset():
        for name,data in originals.items():(root/name).write_bytes(data)
        return {'schema':'conduit.fargan-frozen-repeat-command@1',
                'conformance_reused_from_exact_source':True,
                'conformance_receipt_sha256':v.sha(root/'conformance-reference-status.json'),
                'owners':'prepared-compact-local'}
    status=reset()
    assert v.verify_conformance_receipt(root,status,source,worker)=='reused-exact-source-worker-command'
    def receipt_change(key,value):
        r=copy.deepcopy(receipt);r[key]=value
        (root/'conformance-reference-status.json').write_text(json.dumps(r))
        status['conformance_receipt_sha256']=v.sha(root/'conformance-reference-status.json')
    for name,action in [
        ('missing-reuse-flag',lambda:status.pop('conformance_reused_from_exact_source')),
        ('wrong-receipt-digest',lambda:status.update(conformance_receipt_sha256='0'*64)),
        ('different-source',lambda:receipt_change('git_sha','0'*40)),
        ('failed-conformance',lambda:receipt_change('conformance_success',False)),
        ('failed-worker',lambda:receipt_change('worker_exit_code',1)),
        ('chained-reuse',lambda:receipt_change('conformance_reused_from_exact_source',True)),
        ('different-owner',lambda:receipt_change('owners','fresh')),
        ('changed-proof-log',lambda:(root/'conformance.stdout.log').write_text('forged')),
        ('changed-metadata',lambda:(root/'conformance-reference-metadata.json').write_text('{}')),
        ('missing-proof-log',lambda:(root/'conformance.stderr.log').unlink()),
    ]:
        status=reset();action()
        try:v.verify_conformance_receipt(root,status,source,worker)
        except (AssertionError,KeyError,FileNotFoundError):mutants.append({'mutation':name,'refused':True})
        else:raise AssertionError(f'forgery accepted: {name}')
report={'classification':'conformance receipt verifier controls; no timing or release acceptance',
        'source':source,'worker_sha256':v.sha(worker),'positive_reuse_control':True,
        'mutants':mutants,'verifier_sha256':v.sha(HERE/'verify-compact-local-public.py')}
(HERE/'conformance-receipt-verifier-controls.json').write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps({'positive':True,'mutants_refused':len(mutants)}))
