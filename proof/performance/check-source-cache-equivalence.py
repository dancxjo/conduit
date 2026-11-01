"""Complete-byte differences must refuse even when both captures replay alone."""
from pathlib import Path
import argparse,hashlib,importlib.util,json,os,shutil,tempfile
if not __debug__:
    raise RuntimeError('capture comparison requires assertions; optimized Python is refused')
p=argparse.ArgumentParser();p.add_argument('before',type=Path);p.add_argument('after',type=Path)
for name in ['before-source','after-source','before-worker','after-worker']:p.add_argument('--'+name,required=True,type=Path if name.endswith('worker') else str)
p.add_argument('--output',type=Path);a=p.parse_args()
def module(name):
 spec=importlib.util.spec_from_file_location(name,Path(__file__).with_name(name));m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);return m
comparison=module('verify-source-cache-equivalence.py');single=module('verify-source-cache-public.py')
def verify(after,before_source=a.before_source,after_source=a.after_source):return comparison.verify(a.before,after,a.before_worker,a.after_worker,before_source,after_source)
positive=verify(a.after)
def replace(path,data):
 temp=path.with_name(path.name+'.mutation');temp.write_bytes(data);temp.replace(path)
def corrupt(root,kind):
 rows=[json.loads(s)for s in(root/'raw.jsonl').read_text().splitlines()];changed=[]
 selected=[r for r in rows if r['phase']=='paired-fargan-frame' and r['repetition']==0 and r['frame']==0]
 if kind=='complete-plan-format':
  name='candidate-plan-repetition-0.json';replace(root/name,(root/name).read_bytes()+b'\n');changed.append(name)
 else:
  for row in selected:
   if kind=='complete-events-request' and row['profile']=='fresh':continue
   field='outputs' if kind=='complete-output-format' else 'signs';name=row[field+'_file'];data=(root/name).read_bytes()
   if kind=='complete-output-format':data+=b'\n'
   else:
    assert b'request: None' in data;data=data.replace(b'request: None',b'request: Some(RequestId(1))',1)
   replace(root/name,data);row[field+'_sha256']=hashlib.sha256(data).hexdigest();changed.append(name)
  replace(root/'raw.jsonl',''.join(json.dumps(r)+'\n'for r in rows).encode());changed.append('raw.jsonl')
 status=json.loads((root/'status.json').read_text())
 for name in changed:status['artifact_sha256'][name]=hashlib.sha256((root/name).read_bytes()).hexdigest()
 replace(root/'status.json',(json.dumps(status)+'\n').encode())
results=[]
for kind in ['complete-plan-format','complete-output-format','complete-events-request']:
 with tempfile.TemporaryDirectory(prefix='source-cache-compare-',dir=a.after.resolve().parent)as d:
  root=Path(d)/'capture';shutil.copytree(a.after,root,copy_function=os.link);corrupt(root,kind)
  single.verify(root,a.after_source,a.after_worker)
  try:verify(root)
  except AssertionError:results.append({'case':kind,'independent_capture_replay_passes':True,'complete_comparison_refuses':True})
  else:raise AssertionError(kind)
for kind,before,after in [('wrong-before-source','0'*40,a.after_source),('wrong-after-source',a.before_source,'0'*40)]:
 try:verify(a.after,before,after)
 except AssertionError:results.append({'case':kind,'refused':True})
 else:raise AssertionError(kind)
report={'positive':positive,'refusals':results,'scope':'Independent diagnostic equality only; no timing or release acceptance'}
text=json.dumps(report,indent=2)+'\n'
if a.output:
 with a.output.open('x')as output:output.write(text)
else:print(text,end='')
