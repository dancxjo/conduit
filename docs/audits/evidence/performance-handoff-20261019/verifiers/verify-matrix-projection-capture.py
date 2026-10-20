#!/usr/bin/env python3
"""Replay source custody, deterministic weights, ordered f32 math and every trial."""
import argparse,hashlib,json,statistics,struct
from pathlib import Path
SHAPES=['20x40','40x128','128x160','160x128','192x160','320x160']
INSTALLED=['3x2','18x18','80x1','161x18','192x4','128x40','328x192','192x192','272x480','160x480','240x384','128x384','208x384','160x160','128x128','688x128']
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def f32(v):return struct.unpack('<f',struct.pack('<f',v))[0]
def bits(v):return struct.unpack('<I',struct.pack('<f',v))[0]
def fixture(i,o):
    seed=0x913a6579
    def next_value():
        nonlocal seed
        seed^=(seed<<13)&0xffffffff;seed^=seed>>17;seed^=(seed<<5)&0xffffffff;seed&=0xffffffff
        signed=seed if seed<0x80000000 else seed-0x100000000
        return f32(f32(f32(signed)/f32(2147483647))*f32(.1))
    weights=[next_value() for _ in range(i*o)];inputs=[next_value() for _ in range(i)]
    return weights,inputs
def expected(weights,inputs,i,o,order):
    output=[]
    for row in range(o):
        total=0.
        for column,value in enumerate(inputs):
            index=column*o+row if order=='InputMajor' else row*i+column
            total=f32(total+f32(weights[index]*value))
        output.append(bits(total))
    return output

def verify(root,source,worker):
    status=json.loads((root/'status.json').read_text());meta=json.loads((root/'metadata.json').read_text());summary=json.loads((root/'summary.json').read_text())
    assert status['schema']=='conduit.matrix-projection-performance-command@1'
    assert status['git_sha']==status['final_git_sha']==meta['git_sha']==source
    assert status['source_custody_verified'] and status['conformance_success'] and status['worker_success'] and status['worker_exit_code']==0
    assert meta['source_status']=='clean' and meta['profile']=='release' and not meta['debug_assertions']
    assert meta['worker_sha256']==sha(worker) and meta['binary_bytes']==worker.stat().st_size
    for name,digest in status['artifact_sha256'].items():
        p=Path(name);assert not p.is_absolute() and '..' not in p.parts;assert sha(root/p)==digest,name
    assert {str(p.relative_to(root)) for p in root.rglob('*') if p.is_file()}==set(status['artifact_sha256'])|{'status.json'}
    assert summary['schema']=='conduit.matrix-projection-performance@1' and summary['orders']==2
    shapes=summary.get('shape_list',SHAPES);assert shapes in [SHAPES,INSTALLED+SHAPES,['3x2','80x1']] and summary['shapes']==len(shapes)
    candidate_profile=summary.get('candidate_profile','contiguous');assert candidate_profile in ['contiguous','shape-selected']
    repetitions=summary['repetitions'];iterations=summary['iterations'];assert repetitions==status['repetitions'] and iterations==status['iterations']
    rows=[json.loads(l) for l in (root/'raw.jsonl').read_text().splitlines()];preparation={};trials={}
    for row in rows:
        if row['phase']=='matrix-preparation':
            key=(row['shape'],row['order']);assert key not in preparation;preparation[key]=row
        elif row['phase']=='warm-matrix-projection':
            key=(row['shape'],row['order'],row['repetition'],row['profile']);assert key not in trials;trials[key]=row
            assert row['iterations']==iterations and row['inspection_in_warm_timing'] is False
            counts=row['metrics']['allocator'];assert counts['allocations']==counts['reallocations']==counts['requested_bytes']==0
            assert row['metrics']['wall_ns']>0
    assert len(preparation)==len(shapes)*2 and len(trials)==len(shapes)*2*repetitions*2
    report=[]
    for shape in shapes:
        i,o=map(int,shape.split('x'));weights,inputs=fixture(i,o)
        packed=b''.join(struct.pack('<f',v) for v in weights)
        for order in ['InputMajor','OutputMajor']:
            prep=preparation[shape,order];assert prep['packed_weights_bytes']==i*o*4 and prep['weight_sha256']==hashlib.sha256(packed).hexdigest()
            out=expected(weights,inputs,i,o,order);digest=hashlib.sha256(b''.join(struct.pack('<I',v) for v in out)).hexdigest();ratios=[];cpu_ratios=[]
            sequence=[(r['repetition'],r['profile']) for r in rows if r['phase']=='warm-matrix-projection' and r['shape']==shape and r['order']==order]
            expected_sequence=[]
            for rep in range(repetitions):
                profiles=['scalar-reference',candidate_profile] if rep%2==0 else [candidate_profile,'scalar-reference'];expected_sequence.extend((rep,p) for p in profiles)
                pair={p:trials[shape,order,rep,p] for p in profiles}
                for row in pair.values():
                    assert row['output_bits']==out and row['output_sha256']==digest
                    assert row['multiply_add_pairs']==i*o*iterations
                reference=pair['scalar-reference']['metrics'];candidate=pair[candidate_profile]['metrics'];ratios.append(candidate['wall_ns']/reference['wall_ns'])
                if reference['process_cpu_ns'] and candidate['process_cpu_ns']:cpu_ratios.append(candidate['process_cpu_ns']/reference['process_cpu_ns'])
            assert sequence==expected_sequence
            report.append({'shape':shape,'order':order,'paired_trials':repetitions,'candidate_reference_wall_ratios':ratios,'median_wall_ratio':statistics.median(ratios),'median_process_cpu_ratio':statistics.median(cpu_ratios) if cpu_ratios else None})
    return {'classification':'Independent diagnostic replay; no quiet timing, full Back/Plan/FARGAN, hardware traffic or stable release claim','source':source,'worker_sha256':sha(worker),'weight_shapes_verified':len(shapes)*2,'trials_verified':len(trials),'ordered_f32_outputs_verified':True,'warm_allocation_requests':0,'cases':report}
if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('capture',type=Path);p.add_argument('--source',required=True);p.add_argument('--worker',required=True,type=Path);p.add_argument('--output',type=Path);a=p.parse_args();text=json.dumps(verify(a.capture,a.source,a.worker),indent=2)+'\n'
    if a.output:
        with a.output.open('x') as f:f.write(text)
    else:print(text,end='')
