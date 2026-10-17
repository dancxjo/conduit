import argparse
import hashlib
import json
from pathlib import Path
import statistics
import subprocess

parser=argparse.ArgumentParser(description="Cross-check expanded verified captures and derive source-bound boundary-copy extents.")
parser.add_argument('--verified-comparison',type=Path,required=True)
parser.add_argument('--capture',action='append',type=Path,required=True)
parser.add_argument('--workers',type=Path,required=True)
parser.add_argument('--control',type=Path,required=True)
parser.add_argument('--source-root',type=Path,required=True)
parser.add_argument('--output',type=Path,required=True)
args=parser.parse_args()
assert __debug__ and len(args.capture)==3
workers=json.loads(args.workers.read_text())['workers']
control=json.loads(args.control.read_text())
verified=json.loads(args.verified_comparison.read_text())
assert verified['source']==control['source']
for folder,receipt in zip(args.capture,verified['capture_receipts'],strict=True):
    status=json.loads((folder/'status.json').read_text())
    assert status['artifact_sha256']==receipt['artifact_sha256']
    assert all(hashlib.sha256((folder/f).read_bytes()).hexdigest()==h for f,h in receipt['artifact_sha256'].items())
assert len(workers)==3 and len({w['executable_sha256'] for w in workers})==1
all_plays=set();paired={};datasets=[];copy_rows=[]
for number,capture in enumerate(args.capture):
    rows=[json.loads(l) for l in (capture/'raw.jsonl').read_text().splitlines()]
    current={}
    for r in rows:
        phase=r.get('phase');identity=(phase,r.get('case'),r.get('mode'),r.get('repetition'))
        plays=[];digest=None;input_bytes=None;output_bytes=None
        if phase in ['finite-ordinary-hosted-pipeline','finite-ordinary-fusion-pipeline']:
            plays=[r['active_play']];digest=r['ordered_output_sha256'];input_bytes=64*8;output_bytes=64*8
        elif phase=='bounded-ordinary-stream':
            plays=[r['active_play']];digest=r['output_observations']['ordered_output_sha256'];input_bytes=r['canonical_input_bytes'];output_bytes=r['canonical_output_bytes']
        elif phase=='ordinary-domain-control':
            plays=[r['active_play']];digest=r['ordered_output_sha256'];input_bytes=r['canonical_input_bytes'];output_bytes=r['canonical_output_bytes']
        elif phase=='ordinary-numerical-control':
            plays=[r['active_play']];digest=sorted((v['port'],v['sha256']) for v in r['output_observations']['outputs']);input_bytes=0;output_bytes=sum(len(v['canonical']) for v in r['output_observations']['outputs'])
        elif phase=='ordinary-value-matrix':
            plays=[p['active_play'] for p in r['plays']];digest=r['ordered_output_sha256'];input_bytes=64*8;output_bytes=64*8
        for p in plays:
            assert p['active_play_id'] not in all_plays,('reused actual Play across processes',p)
            all_plays.add(p['active_play_id'])
        if digest is not None:
            current[identity]=(r['source_sha256'],digest)
            copy_rows.append({'capture':number,'case':r['case'],'repetition':r['repetition'],'ingress_store_bytes':input_bytes,'adapter_delivery_bytes':output_bytes,'report_delivery_bytes':output_bytes,'derived_explicit_boundary_copy_extent_bytes':input_bytes+2*output_bytes})
    if paired:assert current==paired,'cross-process workload/output parity differs'
    paired=current;datasets.append(rows)
assert len(all_plays)==3681
assert all(json.loads((p/'status.json').read_text())['git_sha']==control['source'] for p in args.capture)
comparisons=[]
for case in sorted({r['case'] for rows in datasets for r in rows if r.get('phase') in ['ordinary-domain-control','ordinary-numerical-control','ordinary-value-matrix']}):
    groups=[[r for r in rows if r.get('case')==case] for rows in datasets]
    modes=[g[0]['mode'] for g in groups];assert len(set(modes))==1
    samples=[[r['full_host_call']['wall_ns'] for r in g] for g in groups]
    means=[statistics.mean(s) for s in samples]
    comparisons.append({'case':case,'mode':modes[0],'process_samples_ns':samples,'process_means_ns':means,'mean_ns':statistics.mean(means),'sample_sd_process_means_ns':statistics.stdev(means)})
files=['targets/std/src/installed_std.rs','targets/std/src/installed_std/value_fore_outputs.rs','architecture/kernel/src/scheduler/remote_fanout.rs','architecture/kernel/src/scheduler/fanout_value.rs','architecture/kernel/src/hosted.rs']
root=args.source_root
def source_bytes(path):
    return subprocess.check_output(['git','show',control['source']+':'+path],cwd=root)

assert 'bytes: borrowed.to_vec()' in source_bytes(files[0]).decode() and 'let adapter = delivery.clone()' in source_bytes(files[0]).decode()
assert 'report.bytes.extend_from_slice(bytes)' in source_bytes(files[1]).decode() and 'adapter.bytes.extend_from_slice(bytes)' in source_bytes(files[1]).decode()
assert 'let value = values.store(bytes)?' in source_bytes(files[3]).decode() and 'slot.bytes.extend_from_slice(bytes)' in source_bytes(files[4]).decode()
result={'verified_comparison_sha256':hashlib.sha256(args.verified_comparison.read_bytes()).hexdigest(),'source':control['source'],'workers':workers,'verified_distinct_actual_plays':len(all_plays),'verified_matching_workloads':len(paired),'domain_and_value_process_comparisons':comparisons,'copy_extent_source_sha256':{f:hashlib.sha256(source_bytes(f)).hexdigest() for f in files},'source_bound_copy_extents':copy_rows,'copy_scope':'Derived source-level explicit successful ingress-store plus two acknowledged-output owned-buffer payload copies; fan-out shares the admitted input value. Full attempts copy no ingress payload. Not a hardware/total-copy counter: intermediate computation, codec/metadata/Sign/compiler and harness copies remain outside this boundary. No zero-copy claim.','limits':'Three live-observed worker processes with identical executable SHA, CPU 4 affinity, exact source/config/output parity; descriptive process-mean SD, uncontrolled frequency/thermal/background load, no calibrated regression threshold.'}
args.output.write_text(json.dumps(result,indent=2)+'\n')
for row in comparisons:print(row['case'],round(row['mean_ns']/1e6,3),'±',round(row['sample_sd_process_means_ns']/1e6,3),'ms')
print('distinct actual Plays',len(all_plays),'matched workload rows per process',len(paired))
