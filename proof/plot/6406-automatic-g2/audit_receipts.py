from pathlib import Path
import sys
import json,hashlib
root=Path(sys.argv[1]) if len(sys.argv)>1 else Path(__file__).parent/'kernel-receipts';expected={'advance':12,'phases':12,'curve':48,'energy':12};result={'source_commit':'347412941','operations':{},'requests':0,'kernel_events':0,'file_sha256':{}}
assert len(list(root.glob('*.json')))==88
for operation,count in expected.items():
 plan_path=root/f'{operation}-plan.json';plan=json.loads(plan_path.read_text());assert len(plan['fragments'])==1;fragment=plan['fragments'][0];assert len(fragment['placements'])==1;placement=fragment['placements'][0]
 assert placement['implementation_id']=='std/kernel-law-expression@1'
 assert placement['execution_profile_id']=='std/checked-native-law-expression@1'
 assert placement['artifact_id']=='conduit-plot/checked-native-law-expression@1'
 assert placement['kind_id'].startswith('conduitese/law-expression/')
 assert placement['configuration'][0]['key']=='law-program'
 for outer,inner in [('source_document_id','source'),('checked_plot_id','checked'),('expanded_plot_id','expanded')]:assert plan[outer]==fragment[outer]
 requests=list(root.glob(f'{operation}-request-*.json'));assert len(requests)==count
 for i in range(count):
  path=root/f'{operation}-request-{i}.json';q=json.loads(path.read_text());assert q['operation']==operation and q['request_index']==i
  assert q['plan']==plan['plan_id'] and q['gear']==placement['gear_id'] and q['selected_offer']==placement['capability_id']
  for outer,inner in [('source_document_id','source'),('checked_plot_id','checked'),('expanded_plot_id','expanded')]:assert q[inner]==fragment[outer]
  assert q['delivered_output_canonical']==q['expected_output_canonical']
  assert q['value_capacity_before']==q['value_capacity_after']
  events=q['kernel_sign'];kinds=[e['kind'] for e in events]
  for kind in ['HostCallRequested','HostCallCompleted','BackCompleted']:assert kinds.count(kind)==1
  assert not ({'BackFailed','HostCallFailed','RunCancelled','SemanticAbnormal'}&set(kinds))
  requested=next(e for e in events if e['kind']=='HostCallRequested');completed=next(e for e in events if e['kind']=='HostCallCompleted');assert requested['request'] is not None and requested['request']==completed['request'] and requested['node']==completed['node']
  sequence=[e['sequence'] for e in events];assert sequence==sorted(set(sequence))
  result['requests']+=1;result['kernel_events']+=len(events)
 result['operations'][operation]={'requests':count,'plan_id':plan['plan_id'],'kind':placement['kind_id'],'capability':placement['capability_id']}
for path in sorted(root.glob('*.json')):result['file_sha256'][path.name]=hashlib.sha256(path.read_bytes()).hexdigest()
(root.parent/'kernel-receipts-audit.json').write_text(json.dumps(result,indent=2)+'\n');print({k:v for k,v in result.items() if k!='file_sha256'})
