#!/usr/bin/env python3
"""Compare complete frozen captures after independently verifying both commands."""
import argparse,hashlib,importlib.util,json
from pathlib import Path

def load(name):
    p=Path(__file__).with_name(name);spec=importlib.util.spec_from_file_location(name,p);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);return m

def verify(before,after,before_worker,after_worker):
    bm=json.loads((before/'metadata.json').read_text());am=json.loads((after/'metadata.json').read_text())
    load('verify-matrix-local-public.py').verify(before,bm['git_sha'],before_worker)
    load('verify-source-cache-public.py').verify(after,am['git_sha'],after_worker)
    bs=json.loads((before/'summary.json').read_text());as_=json.loads((after/'summary.json').read_text())
    assert bs['source_sha256']==as_['source_sha256'] and bs['repetitions']==as_['repetitions'] and bs['frames_per_repetition']==as_['frames_per_repetition']==24
    def rows(root):
        return {(r['repetition'],r['frame'],r['profile']):r for r in map(json.loads,(root/'raw.jsonl').read_text().splitlines()) if r['phase']=='paired-fargan-frame'}
    old,new=rows(before),rows(after);events=receipts=plans=0
    fields=['nodes','cords','steps','numeric_steps','expression_calls','signs','canonical_intermediates_avoided','canonical_bytes_avoided','canonical_values','canonical_bytes','native_values','native_bytes','logical_invocations','physical_decisions','local_expression_calls','compact_binding_bytes','scheduler_inline_bytes','plan_sha256']
    for r in range(bs['repetitions']):
        for prefix in ['plan-repetition','candidate-plan-repetition']:
            filename=f'{prefix}-{r}.json';assert (before/filename).read_bytes()==(after/filename).read_bytes(),filename;plans+=1
        for f in range(24):
            for bp,ap in [('fresh','fresh'),('prepared-matrix-local','prepared-matrix-source-cache')]:
                a,b=old[r,f,bp],new[r,f,ap]
                for field in fields:assert a[field]==b[field],(r,f,bp,field)
                for kind in ['signs','outputs']:
                    left=(before/a[kind+'_file']).read_bytes();right=(after/b[kind+'_file']).read_bytes();assert left==right,(r,f,bp,kind)
                    if kind=='signs':events+=len(left.splitlines())
                receipts+=1
    return {'schema':'conduit.source-cache-before-after-equivalence@1','before_source':bm['git_sha'],'after_source':am['git_sha'],'all_complete_plan_files_exact':plans,'all_complete_frame_receipts_exact':receipts,'all_complete_events_exact':events,'store_and_work_counters_exact':True,'classification':'independent complete before/after conformance; timing metrics deliberately excluded; no accepted latency, hardware traffic or stable release claim'}
if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('before',type=Path);p.add_argument('after',type=Path);p.add_argument('--before-worker',type=Path,required=True);p.add_argument('--after-worker',type=Path,required=True);p.add_argument('--output',type=Path);a=p.parse_args();result=json.dumps(verify(a.before,a.after,a.before_worker,a.after_worker),indent=2)+'\n'
    if a.output:
        with a.output.open('x') as f:f.write(result)
    else:print(result,end='')
