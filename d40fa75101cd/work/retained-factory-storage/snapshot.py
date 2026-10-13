from pathlib import Path
import json,hashlib,shutil,subprocess
w=Path('work/retained-factory-storage'); m=json.loads(Path('work/model-resource-preparation/immutable-sdk-manifest.json').read_text());m['head']='pending numeric resource owners checkpoint';m['scope']='AI retained Back library graph; no whole driver or trained runtime claim'
for a in list(m['artifacts'].values())+m.get('compiler_proc_macros',[]):
 old=Path(a['path']);current=Path('work/pair-storage-target/debug/deps')/old.name
 if current.exists():
  sha=hashlib.sha256(current.read_bytes()).hexdigest();dest=(w/'immutable-sdk'/sha/current.name).resolve();dest.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(current,dest);a['path']=str(dest);a['sha256']=sha
(w/'immutable-sdk-manifest.json').write_text(json.dumps(m,indent=2)+'\n')
cmd=['rustc','+stable','--edition=2024','--test','semantics/ai/tests/flow_pair_back_preparation.rs','--cfg','feature="fixed-numeric-owners"','-C','debuginfo=0','-o',str(w/'tests')]
for a in m['artifacts'].values():cmd+=['--extern',a['crate']+'='+a['path'],'-L','dependency='+str(Path(a['path']).parent)]
for a in m.get('compiler_proc_macros',[]):cmd+=['-L','dependency='+str(Path(a['path']).parent)]
(w/'command.json').write_text(json.dumps(cmd,indent=2)+'\n')
with (w/'compile.log').open('w')as f:subprocess.run(cmd,stdout=f,stderr=subprocess.STDOUT,check=True)
