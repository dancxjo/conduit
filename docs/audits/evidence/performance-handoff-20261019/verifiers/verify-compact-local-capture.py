#!/usr/bin/env python3
"""Independent diagnostic replay; this does not admit latency or release claims."""
import argparse, collections, hashlib, json, re
from pathlib import Path
EVENT = re.compile(r'KernelEvent \{ sequence: (\d+), node: NodeId\((\d+)\), port: (None|Some\(PortId\(\d+\)\)), request: (None|Some\(RequestId\((\d+)\)\)), kind: ([A-Za-z]+) \}')
TRANSACTIONS = {'ValueConsumed','ValueRouted','BackCompleted','SemanticAbnormal','SemanticAbnormalRecovered'}
def digest(path): return hashlib.sha256(path.read_bytes()).hexdigest()
def event_transactions(path):
    result = collections.defaultdict(list)
    for index, line in enumerate(path.read_text().splitlines()):
        match = EVENT.fullmatch(line); assert match, (path,index)
        sequence,node,port,_,request,kind = match.groups(); assert int(sequence)==index
        assert kind not in {'BackFailed','SemanticAbnormal','RunCancelled','StepFuelExceeded','InputAbnormal'}
        if kind in TRANSACTIONS: result[int(node)].append((port,kind))
    return result,index+1

def verify(root):
    summary=json.loads((root/'summary.json').read_text())
    assert summary['schema']=='conduit.fargan-compact-local-performance@1' and summary['compact_local']
    assert summary['frames_per_repetition']==24
    repetitions=summary['repetitions']; rows=[json.loads(line) for line in (root/'raw.jsonl').read_text().splitlines()]
    phases=collections.Counter(row['phase'] for row in rows)
    for phase,count in [('paired-fargan-frame',48*repetitions),('paired-frame-conformance',24*repetitions),('prepared-rearm',23*repetitions),('paired-evidence-reconstruction-and-write',24*repetitions),('paired-frame-result-release',24*repetitions),('prepared-trajectory-release',repetitions)]: assert phases[phase]==count,(phase,phases[phase])
    frames={}
    for row in rows:
        if row['phase']=='paired-fargan-frame':
            key=(row['repetition'],row['frame'],row['profile']);assert key not in frames; frames[key]=row
            for name in ['signs','outputs']: assert digest(root/row[name+'_file'])==row[name+'_sha256']
            counts=row['metrics']['play']['allocator']; assert counts['allocations']==counts['reallocations']==counts['requested_bytes']==0
        elif row['phase']=='paired-frame-conformance':
            assert row['outputs_equal'] and row['evidence_comparison']=='per-node-ordered-value-transactions'
            assert row['all_signs_equal_with_request_generation_offset'] is None
            assert row['pcm_max_abs_error']<.002 and row['active_state_max_abs_error']<.05 and row['conditioning_history_max_abs_error']<.001
    total=0
    for r in range(repetitions):
        reference=json.loads((root/f'plan-repetition-{r}.json').read_text());candidate=json.loads((root/f'candidate-plan-repetition-{r}.json').read_text())
        for key in ['source_document_id','checked_plot_id','expanded_plot_id']: assert reference[key]==candidate[key]
        for f in range(24):
            fresh=frames[r,f,'fresh'];local=frames[r,f,'prepared-compact-local']
            for row,plan in [(fresh,reference),(local,candidate)]:
                assert row['plan_sha256']==hashlib.sha256(json.dumps(plan,ensure_ascii=False,separators=(',',':')).encode()).hexdigest()
                assert (row['nodes'],row['cords'])==(768,1211)
            assert (root/fresh['outputs_file']).read_bytes()==(root/local['outputs_file']).read_bytes()
            a,ac=event_transactions(root/fresh['signs_file']);b,bc=event_transactions(root/local['signs_file']);assert a==b
            assert (ac,bc)==(6976,6048);total+=ac+bc
            assert local['expression_calls']==0 and local['local_expression_calls']==207
            assert (local['canonical_values'],local['canonical_bytes'],local['native_values'],local['native_bytes'],local['canonical_bytes_avoided'])==(584,990795,140,78336,207056)
            assert (local['logical_invocations'],local['physical_decisions'])==(2858,2546)
    return {'classification':'independent diagnostic conformance replay; no accepted timing or release claim','paired_frames':24*repetitions,'raw_events_parsed':total,'all_final_outputs_exact':True,'per_node_ordered_value_transactions_exact':True,'all_plan_digests_and_source_identities_verified':True,'all_play_allocation_requests':0,'phases':dict(phases)}
if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('capture',type=Path);p.add_argument('--output',type=Path);a=p.parse_args();result=json.dumps(verify(a.capture),indent=2)+'\n'
    if a.output:
        with a.output.open('x') as f:f.write(result)
    else:print(result,end='')
