#!/usr/bin/env python3
"""Independent transport/evidence control replay; no speed or release acceptance."""
import argparse,hashlib,json,re,math,struct
from pathlib import Path

def verify(log,worker,source):
    text=log.read_text()
    assert 'test result: ok. 5 passed; 0 failed;' in text
    assert 'test result: ok. 2 passed; 0 failed;' in text
    assert 'Finished `release` profile [optimized]' in text
    rows=[json.loads(line.split('COMPACT_LINEAR_CONTROL ',1)[1]) for line in text.splitlines() if 'COMPACT_LINEAR_CONTROL ' in line]
    expected={('None',mode) for mode in [0,1,2,3,4]}|{(fault,mode) for fault in ['ResourceVersion','Overflow'] for mode in [0,1,2,4]}
    keyed={(r['fault'],r['mode']):r for r in rows}
    assert len(rows)==len(keyed)==13 and set(keyed)==expected
    ids={(r['source_document_id'],r['plan_id']) for r in rows};assert len(ids)==1 and all(isinstance(s,str) and s for pair in ids for s in pair)
    roles=dict(rows[0]['roles']);assert set(roles)=={'numeric-test/input','numeric-test/weights','numeric-test/sink','numeric/tanh192','numeric/linear192x4','numeric/tanh4'}
    assert set(roles.values())==set(range(6))
    for r in rows:
        assert r['schema']=='conduit.compact-linear-control@2' and dict(r['roles'])==roles
        assert r['play_allocations']==r['play_reallocations']==0
        assert type(r['logical_invocations']) is int and type(r['physical_decisions']) is int
        assert 0<r['physical_decisions']<=r['logical_invocations']
        events=r['events'];assert [e['sequence'] for e in events]==list(range(len(events))) and events
        assert sum(e['kind']=='StepFuelGranted' for e in events)==r['logical_invocations']
        assert all(e['node'] in range(6) and e['port'] in [None,0,1] and e['request'] is None for e in events)
        if r['fault']=='None':
            assert r['failure']=='None' and len(r['output_bits'])==4
            assert all(type(n) is int and 0<=n<2**32 for n in r['output_bits'])
            expected_stores,expected_bytes=(2,784) if r['mode'] in [2,4] else (0,0)
        else:
            assert r['output_bits'] is None and 'BackFailed' in r['failure'] and 'detail: 1403' in r['failure']
            assert sum(e['kind']=='BackFailed' and e['node']==roles['numeric/linear192x4'] for e in events)==1
            assert not any(e['kind']=='ValueRouted' and e['node']==roles['numeric/linear192x4'] for e in events)
            expected_stores,expected_bytes=(1,768) if r['mode'] in [2,4] else (0,0)
        assert (r['compact_stores'],r['raw_bytes'])==(expected_stores,expected_bytes)
    def logical(row,node):return [(e['port'],e['request'],e['kind']) for e in row['events'] if e['node']==node and e['kind']!='StepFuelGranted']
    reference=keyed['None',0];grouped=keyed['None',3]
    values=[struct.unpack('<f',struct.pack('<I',n))[0] for n in reference['output_bits']]
    assert all(math.isfinite(n) and 0<abs(n)<1 for n in values)
    assert values[2]>values[0]>0 and values[3]<values[1]<0
    assert grouped['output_bits']==reference['output_bits'] and grouped['canonical_stores']==reference['canonical_stores']
    for mode in [1,2,4]:
        candidate=keyed['None',mode];control=grouped if mode in [2,4] else reference
        assert candidate['output_bits']==reference['output_bits'] and candidate['events']==control['events']
        assert candidate['physical_decisions']==control['physical_decisions'] and candidate['logical_invocations']==control['logical_invocations']
        assert reference['canonical_stores']-candidate['canonical_stores']==(2 if mode in [2,4] else 0)
        assert all(logical(candidate,n)==logical(reference,n) for n in range(6))
    for fault in ['ResourceVersion','Overflow']:
        reference=keyed[fault,0]
        for mode in [1,2,4]:
            candidate=keyed[fault,mode]
            assert candidate['failure']==reference['failure'] and candidate['events']==reference['events']
            assert reference['canonical_stores']-candidate['canonical_stores']==(1 if mode in [2,4] else 0)
    return {'schema':'conduit.compact-linear-factory-control-replay@1','classification':'Independent retained control replay; no timing, hardware-traffic, full-model matrix admission or release acceptance','source':source,'source_document_id':rows[0]['source_document_id'],'plan_id':rows[0]['plan_id'],'controls':13,'play_allocation_requests':0,'same_schedule_complete_events_equal':True,'unfused_ordered_per_node_logical_events_equal':True,'faults':['ResourceVersion','Overflow'],'factory_control_modes':[4],'successful_compact_intermediates':2,'successful_raw_bytes':784,'matrix_publications_after_fault':0,'log_sha256':hashlib.sha256(log.read_bytes()).hexdigest(),'worker_sha256':hashlib.sha256(worker.read_bytes()).hexdigest(),'limits':'Synthetic finite weights in a real checked typed Plot and existing kernel. Exact owned-offer metadata and Plan-cord resource factory prepare the matrix Back. Caller explicitly binds the DAG; production Host/Plan region selection and full FARGAN matrix-island adoption remain pending. No independent Source-to-Plan recheck.'}
if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('log',type=Path);p.add_argument('--worker',type=Path,required=True);p.add_argument('--source',required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args();result=verify(a.log,a.worker,a.source);a.output.write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))
