#!/usr/bin/env python3
"""Independently inspect every retained frame, ordered Sign and admission phase."""
import argparse,hashlib,json,re,statistics,struct
from pathlib import Path
EVENT=re.compile(r'KernelEvent \{ sequence: (\d+), node: NodeId\((\d+)\), port: (None|Some\(PortId\(\d+\)\)), request: (None|Some\(RequestId\((\d+)\)\)), kind: ([A-Za-z]+) \}')
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def events(p):
 out=[]
 for i,line in enumerate(p.read_text().splitlines()):
  m=EVENT.fullmatch(line);assert m,(p,i,line)
  seq,node,port,_,request,kind=m.groups();assert int(seq)==i
  assert kind not in ['BackFailed','SemanticAbnormal','RunCancelled','StepFuelExceeded','InputAbnormal']
  out.append((int(seq),int(node),port,None if request is None else int(request),kind))
 return out
def active_id(a):
 h=hashlib.sha256()
 for value in ['active-play',a['plan_id'],a['host_id'],a['boot_id']]:
  b=value.encode();h.update(struct.pack('<I',len(b)));h.update(b)
 h.update(struct.pack('<Q',a['play_sequence']));return h.hexdigest()
def verify(path,source):
 status=json.loads((path/'status.json').read_text());assert status['git_sha']==status['final_git_sha']==source
 assert status['source_custody_verified'] and status['conformance_success'] and status['worker_success'] and status['worker_exit_code']==0
 assert status['owners']=='prepared'
 if 'conformance_receipt_sha256' in status:
  reference=Path(status.get('conformance_capture','fargan-prepared-public'))
  assert len(reference.parts)==1 and reference.name not in ['.','..']
  public=path.parent/reference
  assert sha(public/'status.json')==status['conformance_receipt_sha256']
  verify(public,source)
 else:
  proof=(path/'conformance.stdout.log').read_text()
  counts=re.findall(r'test result: ok\. (\d+) passed; 0 failed;',proof)
  assert [int(n) for n in counts]==[6,2]
 for name,digest in status['artifact_sha256'].items():
  relative=Path(name);assert not relative.is_absolute() and '..' not in relative.parts,name
  assert sha(path/relative)==digest,name
 assert {str(p.relative_to(path)) for p in path.rglob('*') if p.is_file()}==set(status['artifact_sha256'])|{'status.json'}
 metadata=json.loads((path/'metadata.json').read_text());assert metadata['git_sha']==source and metadata['source_status']=='clean' and metadata['profile']=='release' and not metadata['debug_assertions']
 summary=json.loads((path/'summary.json').read_text());assert summary['schema']=='conduit.fargan-prepared-performance@1';assert summary['source_sha256']==sha(path/'source.conduit');assert summary['frames_per_repetition']==24
 if source!='f8267b06227f48ed5d3cba645623d53fb7516e2c':assert summary.get('prepared_final_disposal_measured') is True
 rows=[json.loads(line) for line in (path/'raw.jsonl').read_text().splitlines()];repetitions=status['repetitions'];assert repetitions==summary['repetitions']
 expected={(r,f) for r in range(repetitions) for f in range(24)};paired={};conformance={};phases={};plans={}
 for r in range(repetitions):
  plans[r]=json.loads((path/f'plan-repetition-{r}.json').read_text())
 for row in rows:
  phase=row['phase'];r=row.get('repetition');f=row.get('frame');key=(r,f)
  if phase=='paired-fargan-frame':
   profile=row['profile'];assert profile in ['fresh','prepared'];assert (key,profile) not in paired;paired[key,profile]=row
   assert row['nodes']==768 and row['cords']==1211 and row['signs']==6976 and row['steps']==3165
   assert row['canonical_intermediates_avoided']==0
   counts=row['metrics']['play']['allocator'];assert counts['allocations']==counts['reallocations']==counts['requested_bytes']==0
   assert sha(path/row['signs_file'])==row['signs_sha256'];assert sha(path/row['outputs_file'])==row['outputs_sha256']
  elif phase=='paired-frame-conformance':
   assert key not in conformance;conformance[key]=row
   assert row['outputs_equal'] and row['all_signs_equal_with_request_generation_offset']==f
   assert row['pcm_max_abs_error']<.002 and row['active_state_max_abs_error']<.05 and row['conditioning_history_max_abs_error']<.001
   a=row['active'];assert a['play_sequence']==f and a['active_play_id']==active_id(a)
   fragment=plans[r]['fragments'][0]
   for k in ['plan_id','host_id','boot_id']:assert a[k]==fragment[k]
  elif phase in ['cold-prepared-trajectory','prepared-rearm','previous-evidence-release','output-acceptance-state-advance','frame-input-encoding','fresh-call-bindings','prepared-trajectory-release']:
   assert (phase,key) not in phases;phases[phase,key]=row
 assert set(conformance)==expected and set(paired)=={(k,p) for k in expected for p in ['fresh','prepared']}
 comparison=[]
 for key in sorted(expected):
  r,f=key;fresh=paired[key,'fresh'];prepared=paired[key,'prepared']
  plan_digest=hashlib.sha256(json.dumps(plans[r],ensure_ascii=False,separators=(',',':')).encode()).hexdigest()
  assert fresh['plan_sha256']==prepared['plan_sha256']==plan_digest
  assert (path/fresh['outputs_file']).read_bytes()==(path/prepared['outputs_file']).read_bytes()
  a=events(path/prepared['signs_file']);b=events(path/fresh['signs_file']);assert len(a)==len(b)==6976
  for actual,reference in zip(a,b):
   seq,node,port,request,kind=reference;assert actual==(seq,node,port,None if request is None else request+f,kind)
  for field in ['steps','numeric_steps','expression_calls','scheduler_inline_bytes']:assert fresh[field]==prepared[field]
  for phase in ['output-acceptance-state-advance','frame-input-encoding','fresh-call-bindings']:assert (phase,key) in phases
  if f:
   assert phases['prepared-rearm',key]['previous_signs_transferred']==6976
   assert ('previous-evidence-release',key) in phases
  comparison.append({'repetition':r,'frame':f,'fresh_call_ns':fresh['metrics']['whole_call_wall_ns'],'prepared_call_ns':prepared['metrics']['whole_call_wall_ns']})
 for r in range(repetitions):
  assert ('cold-prepared-trajectory',(r,None)) in phases
  assert phases['cold-prepared-trajectory',(r,None)]['invocation_reservation']==24
  if summary.get('prepared_final_disposal_measured'):
   assert ('prepared-trajectory-release',(r,None)) in phases
   disposal=phases['prepared-trajectory-release',(r,None)]['metrics']['allocator']
   assert disposal['allocations']==disposal['reallocations']==disposal['requested_bytes']==0 and disposal['deallocations']>0
 return {'source':source,'frames_verified':len(expected),'ordered_signs_verified':len(expected)*6976*2,'status_sha256':sha(path/'status.json'),'rows':comparison,'median_frame_call_ratio':statistics.median(x['prepared_call_ns']/x['fresh_call_ns'] for x in comparison),'scope':'Exact raw outputs and every Sign; retained request offsets independently checked. Frame ratio excludes separately retained cold preparation, input binding, rearm and acceptance; not total trajectory speedup or hardware memory traffic.'}
def main():
 p=argparse.ArgumentParser();p.add_argument('capture',type=Path);p.add_argument('--source',required=True);p.add_argument('--output',type=Path);a=p.parse_args();result=verify(a.capture,a.source);text=json.dumps(result,indent=2)+'\n'
 if a.output:
  with a.output.open('x') as f:f.write(text)
 else:print(text,end='')
if __name__=='__main__':main()
