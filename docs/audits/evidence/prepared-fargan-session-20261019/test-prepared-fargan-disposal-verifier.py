import pathlib,json,tempfile,shutil,importlib.util,re,hashlib,copy
base=pathlib.Path(__file__).resolve().parent;spec=importlib.util.spec_from_file_location('verifier',base/'verify-prepared-fargan.py');v=importlib.util.module_from_spec(spec);spec.loader.exec_module(v);source='e4fbac12b38ff0f787fe626f4c587cbd175ab5b8';original=base/'fargan-prepared-disposal-public';results=[]
with tempfile.TemporaryDirectory(prefix='fargan-verifier-mutations-',dir=base) as tmp:
 p=pathlib.Path(tmp)/'capture';shutil.copytree(original,p);status=json.loads((p/'status.json').read_text());rows=[json.loads(x) for x in (p/'raw.jsonl').read_text().splitlines()];restores={}
 def save(name):restores.setdefault(name,(p/name).read_bytes())
 def trial(name,change):
  s=copy.deepcopy(status);r=copy.deepcopy(rows);changed=change(s,r)
  if changed is not None:
   save('raw.jsonl');(p/'raw.jsonl').write_text(''.join(json.dumps(x,separators=(',',':'))+'\n' for x in r));changed.add('raw.jsonl')
   for f in changed:s['artifact_sha256'][f]=v.sha(p/f)
  save('status.json');(p/'status.json').write_text(json.dumps(s))
  try:v.verify(p,source)
  except AssertionError:results.append({'mutation':name,'refused':True})
  else:raise AssertionError('accepted invalid capture: '+name)
  for f,b in restores.items():(p/f).write_bytes(b)
 def raw_change(phase,field,value):
  def change(s,r):next(x for x in r if x['phase']==phase)[field]=value;return set()
  return change
 trial('source identity',lambda s,r:s.update(git_sha='wrong'))
 def dirty(s,r):
  save('metadata.json');m=json.loads((p/'metadata.json').read_text());m['source_status']='dirty';(p/'metadata.json').write_text(json.dumps(m));return {'metadata.json'}
 trial('dirty metadata with refreshed checksum',dirty)
 def allocation(s,r):next(x for x in r if x['phase']=='paired-fargan-frame')['metrics']['play']['allocator']['allocations']=1;return set()
 trial('Play allocation with refreshed checksum',allocation)
 trial('oracle error bound',raw_change('paired-frame-conformance','pcm_max_abs_error',1.0))
 trial('cold reservation',raw_change('cold-prepared-trajectory','invocation_reservation',23))
 trial('old evidence count',raw_change('prepared-rearm','previous_signs_transferred',6975))
 def active(s,r):next(x for x in r if x['phase']=='paired-frame-conformance')['active']['active_play_id']='wrong';return set()
 trial('active identity digest',active)
 def missing(s,r):r.remove(next(x for x in r if x['phase']=='previous-evidence-release'));return set()
 trial('missing evidence release',missing)
 def duplicate(s,r):r.append(copy.deepcopy(next(x for x in r if x['phase']=='paired-fargan-frame')));return set()
 trial('duplicate paired frame',duplicate)
 def signs(s,r):
  row=next(x for x in r if x['phase']=='paired-fargan-frame' and x['profile']=='prepared' and x['frame']==1);name=row['signs_file'];save(name);text=(p/name).read_text();text=re.sub(r'RequestId\((\d+)\)',lambda m:'RequestId('+str(int(m[1])+99)+')',text,count=1);(p/name).write_text(text);row['signs_sha256']=v.sha(p/name);return {name}
 trial('correlated request generation with refreshed hashes',signs)
 def order(s,r):
  row=next(x for x in r if x['phase']=='paired-fargan-frame');name=row['signs_file'];save(name);text=(p/name).read_text().splitlines(keepends=True);text[0],text[1]=text[1],text[0];(p/name).write_text(''.join(text));row['signs_sha256']=v.sha(p/name);return {name}
 trial('Sign ordering with refreshed hashes',order)
 def values(s,r):
  row=next(x for x in r if x['phase']=='paired-fargan-frame');name=row['outputs_file'];save(name);data=json.loads((p/name).read_text());data[0][1][-1]^=1;(p/name).write_text(json.dumps(data));row['outputs_sha256']=v.sha(p/name);return {name}
 trial('output mismatch with refreshed hashes',values)
 def plan(s,r):
  name='plan-repetition-0.json';save(name);data=json.loads((p/name).read_text());data['fragments'][0]['host_id']='wrong';(p/name).write_text(json.dumps(data));return {name}
 trial('Plan content with refreshed checksum',plan)
 trial('raw checksum mismatch',lambda s,r:s['artifact_sha256'].update({'raw.jsonl':'0'*64}))
 def missing_final(s,r):r.remove(next(x for x in r if x['phase']=='prepared-trajectory-release'));return set()
 trial('missing final session disposal',missing_final)
 def final_allocation(s,r):next(x for x in r if x['phase']=='prepared-trajectory-release')['metrics']['allocator']['allocations']=1;return set()
 trial('allocation during disposal',final_allocation)
 def false_summary(s,r):
  save('summary.json');data=json.loads((p/'summary.json').read_text());data.pop('prepared_final_disposal_measured');(p/'summary.json').write_text(json.dumps(data));return {'summary.json'}
 trial('removing required final-disposal coverage flag',false_summary)
print(json.dumps({'refusal_cases':len(results),'results':results},indent=2))
