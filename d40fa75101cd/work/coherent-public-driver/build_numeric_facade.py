from pathlib import Path
import json,shutil,subprocess,hashlib
repo=Path(__file__).resolve().parents[2]
project=Path('/home/dancxjo/Documents/Codex/2026-10-06/goal-please-finish-and-close-4907')
old=project/'work/fargan/public-model-driver';source=old/'numeric-owner-facade';dest=repo/'work/coherent-public-driver/numeric-owner'
if not dest.exists():
 shutil.copytree(source,dest)
 for p in dest.rglob('*.rs'):p.write_text(p.read_text().replace(str(source),str(dest)))
 for target,addition,module in [('native_profile.rs','native_profile_storage.rs','bounded_storage'),('operation_owners/native_profile.rs','native_factory_storage.rs','bounded_storage')]:
  p=dest/target;p.write_text(p.read_text()+f'\n#[path="{old/addition}"]\nmod {module};\npub use {module}::*;\n')
 p=dest/'lib.rs';s=p.read_text().replace(str(old/'native_traversal.rs'),str(dest/'native_traversal.rs'));p.write_text(s)
 shutil.copy2(old/'native_traversal.rs',dest/'native_traversal.rs')
 p=dest/'native_traversal.rs';p.write_text(p.read_text()+f'\n#[path="{old/"native_traversal_storage.rs"}"]\nmod bounded_storage;\npub use bounded_storage::*;\n')
manifest=json.loads((repo/'work/coherent-public-driver/coherent-test-sdk-rlibs.json').read_text());args=['rustc','+stable','--edition=2024','--crate-type=rlib','--crate-name','conduit_ai_numeric_union',str(dest/'lib.rs'),'--cfg','feature="kernel-step"','--cfg','feature="kernel-operation-owners"','-L','dependency='+str(repo/'work/pair-storage-target/debug/deps')]
for a in manifest['artifacts'].values():args+=['--extern',('ai_sdk' if a['crate']=='conduit_ai' else a['crate'])+'='+a['path']]
args+=['-C','debuginfo=0','-o',str(dest/'libconduit_ai_numeric_union.rlib')]
(repo/'work/coherent-public-driver/numeric-command.json').write_text(json.dumps(args,indent=2))
with (repo/'work/coherent-public-driver/numeric-build.log').open('w')as log:result=subprocess.run(args,stdout=log,stderr=subprocess.STDOUT)
print('exit',result.returncode);raise SystemExit(result.returncode)
