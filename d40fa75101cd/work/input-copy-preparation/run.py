from pathlib import Path
import json,subprocess,os
w=Path('work/input-copy-preparation');m=json.loads(Path('work/numeric-back-storage/immutable-sdk-manifest.json').read_text());args=[]
for a in m['artifacts'].values():args+=['--extern',a['crate']+'='+a['path'],'-L','dependency='+str(Path(a['path']).parent)]
for a in m.get('compiler_proc_macros',[]):args+=['-L','dependency='+str(Path(a['path']).parent)]
facade=w/'facade.rs';facade.write_text('#![no_std]\nextern crate alloc;\n#[path="../../targets/conduitos/src/fargan_native_profile_preparation.rs"]\npub mod fargan_native_profile_preparation;\n')
commands=[['rustc','+stable','--edition=2024','--crate-name','conduitos','--crate-type','rlib',str(facade),'-C','debuginfo=0','-o',str(w/'libconduitos.rlib')]+args,['rustc','+stable','--edition=2024','--test','targets/conduitos/tests/fargan_native_profile_preparation.rs','--cfg','feature="fargan-native-profile-preparation"','-C','debuginfo=0','--extern','conduitos='+str((w/'libconduitos.rlib').resolve()),'-o',str(w/'tests')]+args]
(w/'commands.json').write_text(json.dumps(commands,indent=2)+'\n')
for i,c in enumerate(commands):
 with(w/f'compile-{i}.log').open('w')as f:subprocess.run(c,stdout=f,stderr=subprocess.STDOUT,check=True)
env=os.environ|{'CONDUIT_FARGAN_PREPARATION_SOURCE_ROOT':'/home/dancxjo/conduit-4898-common-greeting','CONDUIT_FARGAN_PREPARATION_ARTIFACT_ROOT':'/home/dancxjo/Documents/Codex/2026-10-06/goal-please-finish-and-close-4907'}
with(w/'tests.log').open('w')as f:subprocess.run([str(w/'tests'),'--nocapture','--test-threads=1'],env=env,stdout=f,stderr=subprocess.STDOUT,check=True)
