#!/usr/bin/env python3
import argparse,copy,hashlib,importlib.util,json,shutil,tempfile
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('capture');p.add_argument('--source',required=True);a=p.parse_args()
spec=importlib.util.spec_from_file_location('verify_native',Path(__file__).with_name('verify-native-chain.py'));v=importlib.util.module_from_spec(spec);spec.loader.exec_module(v)
capture=Path(a.capture).resolve();v.verify(capture,a.source)
def dump(p,j):p.write_text(json.dumps(j)+'\n')
def row_change(root,change):
 rows=[json.loads(s) for s in (root/'raw.jsonl').read_text().splitlines()]
 target=next(r for r in rows if r.get('phase')=='finite-ordinary-fusion-pipeline' and r['mode']=='native-fused' and r['gears']==32 and r['repetition']==0)
 change(target)
 report=v.read(root/'report.json')
 for i,r in enumerate(report['runs']):
  if r.get('phase')=='finite-ordinary-fusion-pipeline' and r['mode']=='native-fused' and r['gears']==32 and r['repetition']==0:report['runs'][i]=copy.deepcopy(target)
 dump(root/'report.json',report)
 (root/'raw.jsonl').write_text(''.join(json.dumps(r)+'\n' for r in rows))
def file_change(root,name,change):
 path=root/name;j=v.read(path);change(j);dump(path,j)
def duplicate(root):
 rows=[json.loads(s) for s in (root/'raw.jsonl').read_text().splitlines()];r=next(x for x in rows if x.get('mode')=='native-fused')
 rows.append(copy.deepcopy(r));(root/'raw.jsonl').write_text(''.join(json.dumps(x)+'\n' for x in rows))
 report=v.read(root/'report.json');report['runs'].append(r);report['raw_records']=len(rows);dump(root/'report.json',report)
def transient(root):
 def change(r):
  event=next(e for e in r['signs'] if e['kind']=='StepFuelGranted');event['port']=42
 row_change(root,change)
mutations={
 'metadata-debug-mode':lambda d:file_change(d,'metadata.json',lambda j:j.update(debug_assertions=True)),
 'source-custody':lambda d:file_change(d,'status.json',lambda j:j.update(final_git_sha='0'*40)),
 'conformance-refused':lambda d:file_change(d,'conformance.json',lambda j:j.update(success=False)),
 'source-digest':lambda d:row_change(d,lambda r:r.update(source_sha256='0'*64)),
 'selected-profile':lambda d:file_change(d,'sources/pure-chain-32-native-fused-0-plan.json',lambda j:j['fragments'][0]['execution_fusions'][0].update(execution_profile_id='std/pure-chain-dispatch@1')),
 'typed-meaning':lambda d:file_change(d,'sources/pure-chain-32-native-fused-0-plan.json',lambda j:j['fragments'][0]['placements'][0].update(kind_id='value/i64')),
 'canonical-output':lambda d:file_change(d,'sources/pure-chain-32-native-fused-0-outputs.json',lambda j:j[0]['bytes'].__setitem__(0,255)),
 'transient-retained-sign':transient,
 'missing-required-sign':lambda d:row_change(d,lambda r:r.update(signs=[e for i,e in enumerate(r['signs']) if i!=next(n for n,e in enumerate(r['signs']) if e['kind']=='RemoteValueDelivered')])),
 'storage-growth':lambda d:row_change(d,lambda r:r['value_capacity_after'].__setitem__(1,r['value_capacity_after'][1]+16)),
 'hidden-extra-sidecar':lambda d:row_change(d,lambda r:[r[k].__setitem__(1,r[k][1]+16) for k in ('value_capacity_before','value_capacity_after')]),
 'phase-allocation':lambda d:row_change(d,lambda r:r['kernel_phase_wall']['allocator_intervals'][2].update(allocations=r['kernel_phase_wall']['allocator_intervals'][2]['allocations']+1)),
 'missing-cold-phase':lambda d:row_change(d,lambda r:r['cold_phases'].pop()),
 'logical-work':lambda d:row_change(d,lambda r:r.update(kernel_logical_invocations=r['kernel_logical_invocations']+1)),
 'duplicate-profile-row':duplicate,
}
results=[]
for name,change in mutations.items():
 with tempfile.TemporaryDirectory(prefix='native-verifier-mutation-',dir=Path(__file__).parent) as temp:
  root=Path(temp)/'capture';shutil.copytree(capture,root);change(root)
  status=v.read(root/'status.json');status['artifact_sha256']={str(p.relative_to(root)):hashlib.sha256(p.read_bytes()).hexdigest() for p in root.rglob('*') if p.is_file() and p.name!='status.json'};dump(root/'status.json',status)
  try:v.verify(root,a.source)
  except ValueError as e:results.append({'mutation':name,'refused':True,'reason':str(e),'artifact_hashes_refreshed':True})
  else:raise RuntimeError('accepted mutation: '+name)
print(json.dumps({'source':a.source,'semantic_mutations_refused':len(results),'results':results},indent=2))
