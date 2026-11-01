from pathlib import Path
import os,json,hashlib,shutil,tempfile,importlib.util
import argparse
if not __debug__:
    raise RuntimeError('projection replay requires assertions; optimized Python is refused')
parser=argparse.ArgumentParser()
parser.add_argument('capture',type=Path)
parser.add_argument('--source',required=True)
parser.add_argument('--worker',required=True,type=Path)
parser.add_argument('--output',type=Path)
args=parser.parse_args()
source=args.capture.resolve();head=args.source;worker=args.worker.resolve()
candidate=Path(__file__).with_name('verify-matrix-projection-capture.py')
spec=importlib.util.spec_from_file_location('projection',candidate);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
positive=m.verify(source,head,worker)
def write(path,data):
 tmp=path.with_suffix(path.suffix+'.mutation');tmp.write_text(data);tmp.replace(path)
def mutate(root,kind):
 summary=json.loads((root/'summary.json').read_text());rows=[json.loads(x)for x in(root/'raw.jsonl').read_text().splitlines()];meta=json.loads((root/'metadata.json').read_text());status=json.loads((root/'status.json').read_text());warm=next(r for r in rows if r['phase']=='warm-matrix-projection')
 if kind=='wrong-summary-schema':summary['schema']='invented'
 elif kind=='zero-iterations':summary['iterations']=status['iterations']=0
 elif kind=='zero-repetitions':summary['repetitions']=status['repetitions']=0
 elif kind=='boolean-repetitions':summary['repetitions']=status['repetitions']=True
 elif kind=='altered-output-bit':warm['output_bits'][0]^=1
 elif kind=='altered-weight-hash':next(r for r in rows if r['phase']=='matrix-preparation')['weight_sha256']='0'*64
 elif kind=='allocation-request':warm['metrics']['allocator']['allocations']=1
 elif kind=='missing-trial':rows.remove(warm)
 elif kind=='duplicate-trial':rows.append(warm.copy())
 elif kind=='wrong-worker':meta['worker_sha256']='0'*64
 elif kind=='wrong-source':status['git_sha']='0'*40
 for name,data in [('summary.json',summary),('metadata.json',meta)]:write(root/name,json.dumps(data)+'\n')
 write(root/'raw.jsonl',''.join(json.dumps(row)+'\n' for row in rows))
 # Update outer digests so semantic mutations reach the independent arithmetic checks.
 for name in ['summary.json','metadata.json','raw.jsonl']:status['artifact_sha256'][name]=hashlib.sha256((root/name).read_bytes()).hexdigest()
 write(root/'status.json',json.dumps(status)+'\n')
results=[]
for kind in ['wrong-summary-schema','zero-iterations','zero-repetitions','boolean-repetitions','altered-output-bit','altered-weight-hash','allocation-request','missing-trial','duplicate-trial','wrong-worker','wrong-source']:
 with tempfile.TemporaryDirectory(prefix='projection-refusal-',dir=source.parent) as d:
  root=Path(d)/'capture';shutil.copytree(source,root,copy_function=os.link);mutate(root,kind)
  try:m.verify(root,head,worker)
  except (AssertionError,KeyError,ValueError,FileNotFoundError) as e:results.append({'case':kind,'refused':True,'error_class':type(e).__name__})
  else:raise AssertionError(kind)
report={'positive':{k:v for k,v in positive.items()if k!='cases'},'refusals':results,'manifest_hashes_recomputed_for_semantic_mutations':True,'candidate_sha256':hashlib.sha256(candidate.read_bytes()).hexdigest(),'scope':'Fresh source-bound diagnostic arithmetic replay; no speed/timing acceptance'}
if args.output:
 with args.output.open('x') as output: output.write(json.dumps(report,indent=2)+'\n')
else: print(json.dumps(report,indent=2))
