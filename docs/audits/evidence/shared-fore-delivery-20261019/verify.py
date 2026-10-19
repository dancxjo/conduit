#!/usr/bin/env python3
"""Verify custody and compare the ordinary owned-adapter release cases.

Fresh output directories intentionally mint fresh Boot/Plan/active identities.
This verifier checks those correlations within each capture and compares exact
ordered kernel events, values, source and work across captures. It does not
claim byte-identical Plans across distinct Boots or compare unrelated cases.
"""
import argparse,hashlib,json,statistics
from pathlib import Path
SOURCES={'before':'99dfe6eceeb9f8e318e388a06beb98382746c647','after':'f2bb3b0b9640c8a9f2ccd2626af817538706c7e5'}
FIELDS=['source_sha256','ordered_output_sha256','gears','values','adds_per_value','completion_window_items',
        'kernel_decisions','kernel_logical_invocations','retained_kernel_events','signs','value_capacity_before','value_capacity_after']
def digest(p):
 h=hashlib.sha256()
 with p.open('rb') as f:
  for b in iter(lambda:f.read(1024*1024),b''):h.update(b)
 return h.hexdigest()
def load(path,side):
 receipt_path=path/('capture-status.json' if (path/'capture-status.json').exists() else 'status.json')
 receipt=json.loads(receipt_path.read_text())
 if 'worker_success' in receipt:
  assert receipt['worker_success'] and receipt['conformance_success']
  assert receipt['git_sha']==receipt['final_git_sha']==SOURCES[side]
 else:
  assert receipt['exit_code']==0 and receipt['source']==SOURCES[side]
 assert receipt['source_custody_verified']
 for relative,sha in receipt['artifact_sha256'].items(): assert digest(path/relative)==sha,relative
 metadata=json.loads((path/'metadata.json').read_text())
 assert metadata['git_sha']==SOURCES[side] and metadata['source_status']=='clean'
 assert metadata['profile']=='release' and not metadata['debug_assertions']
 report=json.loads((path/'report.json').read_text())
 arguments=receipt.get('worker_arguments',receipt.get('command'))
 recorded_output=report['config']['output']
 assert arguments[arguments.index('--output')+1]==recorded_output
 raw=[json.loads(line) for line in (path/'raw.jsonl').open()]
 assert report['raw_records']==len(raw)
 rows={}
 expected_allocation={'allocations':378 if side=='before' else 63,'deallocations':0,
                      'reallocations':0,'requested_bytes':2898 if side=='before' else 1512}
 for row in raw:
  if row.get('phase')!='finite-ordinary-hosted-pipeline':continue
  key=(row['case'],row['repetition']); assert key not in rows
  assert row['post_first_output_allocations']==expected_allocation
  assert row['values']==64 and row['completion_window_items']==63 and row['adds_per_value']==32
  assert row['value_capacity_before']==row['value_capacity_after']
  active=row['active_play']
  assert active['plan_id']==row['plan_id'] and active['host_id']=='host/runtime-performance'
  expected_boot=f"boot/runtime-performance/{hashlib.sha256(recorded_output.encode()).hexdigest()}/{row['gears']}/{row['repetition']}"
  assert active['boot_id']==expected_boot
  assert len(row['observations'])==1
  obs=row['observations'][0]
  for name in ['active_play_id','boot_id','host_id','plan_id']:assert obs[name]==active[name]
  assert obs['kind']=={'PlanTerminal':{'disposition':'Completed'}}
  assert obs['placement_id'] is None and obs['connection_id'] is None and obs['face_id'] is None
  rows[key]=row
 assert len(rows)==9
 assert {(r['gears'],r['repetition']) for r in rows.values()}=={(g,r) for g in [1,8,32] for r in range(3)}
 return rows,metadata,receipt_path

def compare(before,after):
 b,bm,br=load(before,'before');a,am,ar=load(after,'after')
 for field in ['compiler','cargo','profile','debug_assertions','rustflags','release_overrides']:
  assert bm[field]==am[field],field
 assert b.keys()==a.keys()
 rows=[]
 for key,x in sorted(a.items()):
  y=b[key]
  for field in FIELDS:assert x[field]==y[field],(key,field)
  rows.append({'case':key[0],'repetition':key[1],'gears':x['gears'],
   'output_sha256':x['ordered_output_sha256'],'ordered_kernel_signs_sha256':hashlib.sha256(json.dumps(x['signs'],sort_keys=True,separators=(',',':')).encode()).hexdigest(),
   'before_allocation':y['post_first_output_allocations'],'after_allocation':x['post_first_output_allocations'],
   'full_host_call_ratio':x['full_host_call']['wall_ns']/y['full_host_call']['wall_ns'],
   'completion_window_ratio':x['first_to_last_acknowledgement_ns']/y['first_to_last_acknowledgement_ns'],
   'before_full_host_call_ns':y['full_host_call']['wall_ns'],'after_full_host_call_ns':x['full_host_call']['wall_ns'],
   'before_completion_ns':y['first_to_last_acknowledgement_ns'],'after_completion_ns':x['first_to_last_acknowledgement_ns']})
 return {'before':str(before),'after':str(after),'before_receipt_sha256':digest(br),'after_receipt_sha256':digest(ar),
  'scope':'9 ordinary owned-adapter cases; exact Source/output/ordered kernel events/work/capacities; fresh Boot/Plan/active identities checked within capture, not claimed identical across Boots. Descriptive timings; public capture may overlap compilation. Full matrix retained but other cases not compared by this verifier.',
  'rows':rows,'median_ratios_by_gears':{str(g):{name:statistics.median(r[name] for r in rows if r['gears']==g) for name in ['full_host_call_ratio','completion_window_ratio']} for g in [1,8,32]}}
def main():
 p=argparse.ArgumentParser();p.add_argument('before',type=Path);p.add_argument('after',type=Path);p.add_argument('--output',type=Path);args=p.parse_args()
 result=compare(args.before.resolve(),args.after.resolve()); text=json.dumps(result,indent=2)+'\n'
 if args.output:
  with args.output.open('x') as f:f.write(text)
 else:print(text,end='')
if __name__=='__main__':main()
