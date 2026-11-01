"""Run one real positive replay, then reject independently corrupted copies."""
import hashlib,importlib.util,json,os,shutil,tempfile
from pathlib import Path
if not __debug__:
    raise RuntimeError('matrix-local replay requires assertions; optimized Python is refused')
import argparse
parser=argparse.ArgumentParser()
parser.add_argument('capture',type=Path)
parser.add_argument('--source',required=True)
parser.add_argument('--worker',required=True,type=Path)
parser.add_argument('--output',type=Path)
args=parser.parse_args()
capture=args.capture.resolve()
script=Path(__file__).with_name('verify-matrix-local-capture.py')
spec=importlib.util.spec_from_file_location('candidate',script);verifier=importlib.util.module_from_spec(spec);spec.loader.exec_module(verifier)
public_spec=importlib.util.spec_from_file_location('public',Path(__file__).with_name('verify-matrix-local-public.py'))
public=importlib.util.module_from_spec(public_spec);public_spec.loader.exec_module(public)
positive=public.verify(capture,args.source,args.worker)
assert positive['paired_frames'] >= 48
# Only replace files in scratch; never write through a hardlink to the capture.
def replace_json(path,data):
 tmp=path.with_name(path.name+'.mutation');tmp.write_text(json.dumps(data)+'\n');tmp.replace(path)
def mutate_summary(root,key,value):
 path=root/'summary.json';data=json.loads(path.read_text());data[key]=value;replace_json(path,data)
def mutate_rows(root,fn):
 path=root/'raw.jsonl';rows=[json.loads(s) for s in path.read_text().splitlines()];fn(rows)
 tmp=path.with_name('raw.mutation');tmp.write_text(''.join(json.dumps(row)+'\n' for row in rows));tmp.replace(path)
def selected(rows):return next(r for r in rows if r['phase']=='paired-fargan-frame')
cases=[('zero-repetitions',lambda r:mutate_summary(r,'repetitions',0)),('one-repetition',lambda r:mutate_summary(r,'repetitions',1)),('wrong-schema',lambda r:mutate_summary(r,'schema','invented')),('wrong-frame-count',lambda r:mutate_summary(r,'frames_per_repetition',23)),('allocation-request',lambda r:mutate_rows(r,lambda rows:selected(rows)['metrics']['play']['allocator'].update(allocations=1))),('corrupt-sign-digest',lambda r:mutate_rows(r,lambda rows:selected(rows).update(signs_sha256='0'*64))),('corrupt-output-digest',lambda r:mutate_rows(r,lambda rows:selected(rows).update(outputs_sha256='0'*64))),('missing-frame-row',lambda r:mutate_rows(r,lambda rows:rows.remove(selected(rows)))),('duplicate-frame-row',lambda r:mutate_rows(r,lambda rows:rows.append(selected(rows).copy())))]
results=[]
for name,mutate in cases:
 with tempfile.TemporaryDirectory(prefix='matrix-refusal-',dir=capture.parent) as directory:
  root=Path(directory)/'capture';shutil.copytree(capture,root,copy_function=os.link);mutate(root)
  try:verifier.verify(root)
  except (AssertionError,KeyError,ValueError,FileNotFoundError) as error:results.append({'case':name,'refused':True,'error_class':type(error).__name__})
  else:raise AssertionError('corruption admitted: '+name)
report={'positive':positive,'refusals':results,'verifier_sha256':hashlib.sha256(script.read_bytes()).hexdigest(),'source':args.source,'scope':'Actual current capture replay and scratch mutation controls; no timing/release acceptance'}
if args.output:
 with args.output.open('x') as output: output.write(json.dumps(report,indent=2)+'\n')
else: print(json.dumps(report,indent=2))
