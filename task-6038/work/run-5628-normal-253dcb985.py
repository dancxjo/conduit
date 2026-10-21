from pathlib import Path
import subprocess,os,json,hashlib,shutil,time
T=Path('/home/dancxjo/Documents/Codex/2026-10-19/goal-6038');W=T/'work/selected-look-reconciled'
head='253dcb985eb8890ddf873d672d3a282f2366e6db'
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=W,text=True).strip()==head
assert not subprocess.check_output(['git','status','--porcelain'],cwd=W,text=True).strip()
source=W/'target/conduitos/live/x86_64-pc'; product=T/'outputs/5628-product-253dcb985'
m=json.loads((source/'build-manifest.json').read_text())
assert m['source_identity']==head and m['artifact_role']=='product-host'
for key in ['kernel','image']:
 a=m[key];assert hashlib.sha256((source/a['file']).read_bytes()).hexdigest()==a['sha256']
assert not product.exists()
shutil.copytree(source,product)
env=dict(os.environ,CARGO_TARGET_DIR=str(T/'work/native-panels-host-target'),CARGO_BUILD_JOBS='4',CARGO_PROFILE_DEV_DEBUG='0',CARGO_INCREMENTAL='0')
with (T/'work/5628-normal-253dcb985.log').open('wb') as log:
 subprocess.run(['cargo','+stable','xtask','make','host','verify',str(product),'--journey'],cwd=W,env=env,stdout=log,stderr=subprocess.STDOUT,check=True)
packet=W/'target/conduitos/x86_64'; dest=T/'outputs/5628-normal-journey-253dcb985/x86_64'
assert (packet/'journey-proof.json').is_file() and not dest.exists()
dest.mkdir(parents=True)
for name in ['journey-serial.log','native-shell-paired-linear.json','memory-lantern-native-edit.json','journey-qemu-stderr.log','journey-proof.json','journey-qmp.log','x86_64-product-journey-proof.json']:
 assert (packet/name).is_file(),name
 shutil.copy2(packet/name,dest/name)
shutil.copytree(packet/'journey-frames',dest/'journey-frames')
print('Supported normal journey passed; packet retained',flush=True)
