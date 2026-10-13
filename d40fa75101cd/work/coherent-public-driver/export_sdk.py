from pathlib import Path
import json, hashlib, subprocess, sys
root=Path(__file__).resolve().parents[2];target=root/'work/pair-storage-target/debug';fp=target/'.fingerprint';deps=target/'deps'
seen={};excluded=[]
def visit(name,marker=None,hashname=None):
 if marker is not None:
  candidates=[p for p in fp.glob('*/lib-'+name) if p.read_text().strip()==marker]
 else:candidates=[fp/f'{name.replace("_","-")}-{hashname}'/('lib-'+name)]
 candidates=[p for p in candidates if (deps/f'lib{name}-{p.parent.name.rsplit("-",1)[1]}.rlib').exists()]
 if len(candidates)!=1:raise RuntimeError((name,marker,[str(p) for p in candidates]))
 p=candidates[0];key=p.parent.name
 if key in seen:return
 h=key.rsplit('-',1)[1];lib=deps/f'lib{name}-{h}.rlib';metadata=json.loads(p.with_suffix('.json').read_text());edges=[]
 seen[key]={'crate':name,'path':str(lib),'sha256':hashlib.sha256(lib.read_bytes()).hexdigest(),'fingerprint':p.read_text().strip(),'features':metadata['features'],'deps':edges}
 for _,child,_,value in metadata['deps']:
  if child=='build_script_build':continue
  mark=value.to_bytes(8,'little').hex()
  try:visit(child,mark)
  except RuntimeError:
   matches=[x for x in fp.glob('*/lib-'+child) if x.read_text().strip()==mark]
   if matches and any((deps/f'lib{child}-{x.parent.name.rsplit("-",1)[1]}.so').exists() for x in matches):excluded.append({'crate':child,'fingerprint':mark});continue
   raise
  edges.append({'crate':child,'fingerprint':mark})
if '--public' in sys.argv:
 rows=[json.loads(l) for l in (root/'work/coherent-public-driver/public-ai-build.jsonl').read_text().splitlines()]
 artifact=next(r for r in rows if r.get('reason')=='compiler-artifact' and r['target']['name']=='conduit_ai')
 hashed=next(Path(p).name for p in artifact['filenames'] if p.endswith('.rmeta'))
 selected_hash=hashed.rsplit('-',1)[1].split('.')[0]
else:selected_hash='53f08a7072359bc2' if '--test' in sys.argv else '5cb28f8b1c7e346f'
visit('conduit_ai',hashname=selected_hash)
source={}
for name in ['architecture/core','architecture/plot','architecture/composite','architecture/kernel','architecture/plan-lowering','semantics/ai','semantics/data','semantics/time']:
 for rel in subprocess.check_output(['git','ls-files','--cached','--others','--exclude-standard',name],cwd=root,text=True).splitlines():
  p=root/rel
  if ('/src/' in rel or rel.endswith('/build.rs') or rel.endswith('Cargo.toml')) and p.is_file():source[rel]=hashlib.sha256(p.read_bytes()).hexdigest()
out={'head':subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip(),'root':str(root),'scope':'AI library coherent union only; no FARGAN new factory/runtime claim','artifacts':seen,'excluded_proc_macros':excluded,'source_files':source}
(root/('work/coherent-public-driver/coherent-test-sdk-rlibs.json' if '--test' in sys.argv else ('work/coherent-public-driver/coherent-public-sdk-rlibs.json' if '--public' in sys.argv else 'work/coherent-public-driver/coherent-sdk-rlibs.json'))).write_text(json.dumps(out,indent=2)+'\n')
print(len(seen),'rlibs;',len(source),'Source files')
