"""Read-only verification of a complete separately pinned diagnostic replay."""
from pathlib import Path
import json,hashlib,sys,collections
root=Path(sys.argv[1]).resolve(); admission=Path(sys.argv[2]).resolve(); output=Path(sys.argv[3])
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()

def verify_commit_observation(o):
 # Independently mirror frozen Source agreement/advance and actual commit application.
 c=o['commit']; before=c['before']; after=c['after']; active=[h for h in before if h['active']]
 assert len(before)==len(after)==4 and active
 assert after==o['occupancy']['all_four']
 initial=before[0]['committed']; stable=initial; halted=False
 assert len(c['agreements'])==o['available_count']
 for ordinal,a in enumerate(c['agreements']):
  ref=active[0]
  edge=lambda h:(h['heads'][ordinal],h['relations'][ordinal],h['relation_subtypes'][ordinal])
  agrees=ref['heads'][ordinal]<5 and all(edge(h)==edge(ref) for h in active)
  assert a['ordinal']==ordinal and a['survivors']==len(active)
  assert a['head']==ref['heads'][ordinal] and a['relation']==ref['relations'][ordinal]
  assert a['agrees']==agrees
  if not halted and ordinal==stable:
   if agrees and stable<4: stable+=1
   else: halted=True
 assert c['stable']==stable
 through=c['pending_consumer_through']
 requested=through is not None and initial<through<=stable
 assert c['actual_result_applied_to_current_beam']==requested
 assert not c['accepted'] or requested
 assert c['committed']==(through if c['accepted'] else initial)
 for old,new in zip(before,after):
  expected=dict(old)
  if c['accepted'] and old['active']: expected['committed']=through
  assert new==expected, 'commit changed fields beyond accepted active frontier'
 for h in before:
  assert len(h['choices'])==len(h['relations'])==len(h['relation_subtypes'])==4
  assert len(h['heads'])==5 and 0<=h['committed']<=4 and 0<=h['selected']<=4
  assert all(isinstance(v,int) and 0<=v<4 for v in h['choices'])
 return len(c['agreements'])
manifest=json.load(open(admission/'pair-manifest.json'));cases=json.load(open(manifest['cases']['path']))['cases']
expected={f'{arm}-{i:02}-report.json' for arm in ['causal','full'] for i in range(17)}
assert {p.name for p in root.glob('*-report.json')}==expected,'incomplete34report set'
full=json.load(open(root/'report.json'));assert full['completed_batches']==34 and not full['accepted_default'] and not full['installed_current_controller']
assert full['manifest']==manifest
receipt={'classification':'UNACCEPTED_NONBLIND_REPLACEMENT_DIAGNOSTIC_NOT_INSTALLED_ACCEPTANCE','original29_reused_in_aggregate':False,'manifest_sha256':sha(admission/'pair-manifest.json'),'source_pins_sha256':sha(admission/'source-pins.json'),'replay_plan':json.load(open(admission/'replay-plan.json')),'complete_report_sha256':sha(root/'report.json'),'arms':{},'trace_files':{},'verification':{'complete_case_arm_reports':34,'same_frozen_cases_models_schedule':True,'numerical_receipts_checked':0,'all_four_survivors_checked':0,'source_trace_records_checked':0}}
for arm in ['causal','full']:
 rows=[]
 for i,case in enumerate(cases):
  path=root/f'{arm}-{i:02}-report.json';r=json.load(open(path));c=r['case'];assert r['status']=='CASE_COMPLETED' and r['arm']==arm and r['case_ordinal']==i and c['id']==case['id']
  assert r['identity']['sha256']==manifest['arms'][arm]['sha256']
  assert len(r['numerical_receipts'])==r['model_invocations']
  for n,ack in enumerate(r['numerical_receipts'],1):
   assert ack['invocation']==n and ack['model_content']==r['identity']['content'] and ack['work_units']==1900
   assert len(ack['input'])==len(ack['output'])==32
  receipt['verification']['numerical_receipts_checked']+=len(r['numerical_receipts'])
  assert len(c['observations'])==len(case['steps'])
  for j,(o,s) in enumerate(zip(c['observations'],case['steps'])):
   assert o['step']==j and o['text']==s['text'] and o['available_count']==s['expected_available_count'] and o['source_finality']=='partial'
   assert o['prefinal_observation']==(j+1<len(case['steps']))
   assert len(o['occupancy']['all_four'])==4
   receipt['verification']['all_four_survivors_checked']+=1
   for h in o['occupancy']['all_four']:
    assert len(h['choices'])==4 and len(h['heads'])==5 and len(h['relations'])==4
   receipt['verification']['agreement_frontier_records_checked']=receipt['verification'].get('agreement_frontier_records_checked',0)+verify_commit_observation(o)
  traces=[r['preparation_source_trace']]+r['prefix_source_traces']
  for tr in traces:
   p=Path(tr['path']);assert p.parent==root and p.stat().st_size==tr['end_exclusive'] and tr['start_inclusive']==0 and 0<p.stat().st_size<=67108864
   entries=collections.Counter(); count=0
   with p.open() as f:
    for line in f:
     v=json.loads(line); assert isinstance(v['sequence'],int) and v['sequence']>=0
     bytes.fromhex(v['input_canonical_hex']);bytes.fromhex(v['output_canonical_hex']);entries[v['entry']]+=1;count+=1
   assert count>0
   receipt['trace_files'][p.name]={'sha256':sha(p),'bytes':p.stat().st_size,'records':count,'entries':dict(entries)}
   receipt['verification']['source_trace_records_checked']+=count
  row={k:c[k] for k in ['id','tokens','uas_correct','las_correct','vocative_tp','vocative_fp','vocative_fn','nonfinal_commit','first_commit_step']}
  row.update({'report_sha256':sha(path),'model_invocations':r['model_invocations'],'illegal_candidates_refused':r['illegal'],'refused_expansions':r['refused_expansions'],'accepted_commits':sum(bool(o['commit']['accepted']) for o in c['observations']),'changes_after_commit':sum(s['changes_after_commit'] for s in c['stability']),'winner_edge_revisions':sum(s['winner_edge_revisions'] for s in c['stability']),'observed_prefix_elapsed_ns':sum(o['elapsed_ns'] for o in c['observations'])})
  rows.append(row)
 totals={k:sum(r[k] for r in rows) for k in ['tokens','uas_correct','las_correct','vocative_tp','vocative_fp','vocative_fn','model_invocations','illegal_candidates_refused','accepted_commits','changes_after_commit','winner_edge_revisions','observed_prefix_elapsed_ns']}
 totals['nonfinal_committing_cases']=sum(r['nonfinal_commit'] for r in rows);receipt['arms'][arm]={'totals':totals,'cases':rows}
output.write_text(json.dumps(receipt,indent=2)+'\n');print(json.dumps(receipt['verification']));print(json.dumps({a:r['totals'] for a,r in receipt['arms'].items()}))
