#!/usr/bin/env python3
"""Independent diagnostic replay; this does not admit latency or release claims."""
import argparse, collections, hashlib, json, re
from pathlib import Path
EVENT = re.compile(r'KernelEvent \{ sequence: (\d+), node: NodeId\((\d+)\), port: (None|Some\(PortId\(\d+\)\)), request: (None|Some\(RequestId\((\d+)\)\)), kind: ([A-Za-z]+) \}')
TRANSACTIONS = {'ValueConsumed','ValueRouted','BackCompleted','SemanticAbnormal','SemanticAbnormalRecovered'}
def digest(path): return hashlib.sha256(path.read_bytes()).hexdigest()
def event_transactions(path, forbid_host=False):
    result = collections.defaultdict(list)
    for index, line in enumerate(path.read_text().splitlines()):
        match = EVENT.fullmatch(line); assert match, (path,index)
        sequence,node,port,_,request,kind = match.groups(); assert int(sequence)==index
        assert kind not in {'BackFailed','SemanticAbnormal','RunCancelled','StepFuelExceeded','InputAbnormal'}
        assert 0 <= int(node) < 768
        if forbid_host: assert not kind.startswith('HostCall')
        if kind in TRANSACTIONS: result[int(node)].append((port,kind))
    return result,index+1

def verify(root):
    summary=json.loads((root/'summary.json').read_text())
    assert summary['schema']=='conduit.fargan-matrix-local-performance@1' and summary['compact_local']
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
        fragment=candidate['fragments'][0]
        assert len(candidate['fragments'])==1 and not reference['fragments'][0]['execution_fusions']
        assert len(fragment['placements'])==768 and len(fragment['connections'])==1211
        assert all(not p['host_calls'] for p in fragment['placements'])
        placements={p['placement_id']:p for p in fragment['placements']}
        assert len(placements)==768
        groups=fragment['execution_fusions']; assert len(groups)==32
        seen=set();sizes=collections.Counter();linear=slices=0
        for group in groups:
            assert group['execution_profile_id']=='conduit.numeric/finite-f32-matrix-dag@1'
            assert group['implementation_id']=='conduit.numeric/compact-matrix-dag@1'
            assert group['artifact_id']=='conduit-kernel/logical-step-f32-matrix-dag@1'
            members=group['preserved_placements']; assert 2<=len(members)<=32 and len(set(members))==len(members)
            assert not seen.intersection(members); seen.update(members);sizes[len(members)]+=1
            for member in members:
                p=placements[member]
                assert p['host_id']==fragment['host_id'] and p['boot_id']==fragment['boot_id'] and p['offer_generation']==fragment['offer_generation']
                linear+=p['kind_id'].startswith('numeric/linear')
                slices+=p['kind_id'] in {'numeric/slice480x160','numeric/slice384x128'}
        assert sizes=={2:20,19:12} and linear==24 and slices==72
        for f in range(24):
            fresh=frames[r,f,'fresh'];local=frames[r,f,'prepared-matrix-local']
            for row,plan in [(fresh,reference),(local,candidate)]:
                assert row['plan_sha256']==hashlib.sha256(json.dumps(plan,ensure_ascii=False,separators=(',',':')).encode()).hexdigest()
                assert (row['nodes'],row['cords'])==(768,1211)
            assert (root/fresh['outputs_file']).read_bytes()==(root/local['outputs_file']).read_bytes()
            a,ac=event_transactions(root/fresh['signs_file']);b,bc=event_transactions(root/local['signs_file'],True);assert a==b
            assert (ac,bc)==(6976,5973);total+=ac+bc
            assert (fresh['canonical_values'],fresh['canonical_bytes'],fresh['native_values'],fresh['expression_calls'])==(724,1197851,0,207)
            assert local['expression_calls']==0 and local['local_expression_calls']==207
            assert (local['canonical_values'],local['canonical_bytes'],local['native_values'],local['native_bytes'],local['canonical_bytes_avoided'])==(488,789963,236,158208,407888)
            assert (local['logical_invocations'],local['physical_decisions'])==(2783,2183)
    return {'classification':'independent diagnostic conformance replay; no accepted timing or release claim','paired_frames':24*repetitions,'raw_events_parsed':total,'all_final_outputs_exact':True,'per_node_ordered_value_transactions_exact':True,'all_plan_digests_and_source_identities_verified':True,'all_play_allocation_requests':0,'phases':dict(phases)}
if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('capture',type=Path);p.add_argument('--output',type=Path);a=p.parse_args();result=json.dumps(verify(a.capture),indent=2)+'\n'
    if a.output:
        with a.output.open('x') as f:f.write(result)
    else:print(result,end='')
