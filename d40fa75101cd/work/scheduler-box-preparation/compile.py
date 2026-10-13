from pathlib import Path
import json,hashlib,shutil,subprocess
w=Path('work/scheduler-box-preparation');artifacts=[]
for line in (w/'artifacts.jsonl').read_text().splitlines():
 a=json.loads(line)
 if a.get('reason')!='compiler-artifact':continue
 for filename in a['filenames']:
  p=Path(filename)
  if p.suffix not in ['.rlib','.so']:continue
  sha=hashlib.sha256(p.read_bytes()).hexdigest();dest=(w/'immutable-sdk'/sha/p.name).resolve();dest.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(p,dest)
  artifacts.append({'crate':a['target']['name'],'path':str(dest),'sha256':sha,'features':a['features'],'kind':p.suffix})
(w/'immutable-sdk-manifest.json').write_text(json.dumps({'scope':'fresh exact Kernel-only Cargo graph; not AI graph','artifacts':artifacts},indent=2)+'\n')
args=[]
for a in artifacts:
 args+=['-L','dependency='+str(Path(a['path']).parent)]
 if a['kind']=='.rlib':args+=['--extern',a['crate']+'='+a['path']]
c=['rustc','+stable','--edition=2021','--test','architecture/kernel/tests/scheduler_box_storage.rs','--cfg','feature="alloc"','-C','debuginfo=0','-o',str(w/'tests')]+args;(w/'command.json').write_text(json.dumps(c,indent=2)+'\n')
with(w/'compile.log').open('w')as f:subprocess.run(c,stdout=f,stderr=subprocess.STDOUT,check=True)
