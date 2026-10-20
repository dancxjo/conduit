from pathlib import Path
import subprocess,os,json,hashlib,shutil,time
T=Path('/home/dancxjo/Documents/Codex/2026-10-19/goal-6038');W=T/'work/native-typography'
head='3860faa7e51d0ca8c85f244cbdf9b0101877981f'
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=W,text=True).strip()==head
assert not subprocess.check_output(['git','status','--porcelain'],cwd=W,text=True).strip()
source=W/'target/conduitos/live/x86_64-pc'; product=T/'outputs/5992-product-3860faa7e'
m=json.loads((source/'build-manifest.json').read_text())
assert m['source_identity']==head and m['artifact_role']=='product-host'
for key in ['kernel','image']:
 a=m[key];assert hashlib.sha256((source/a['file']).read_bytes()).hexdigest()==a['sha256']
assert not product.exists()
shutil.copytree(source,product)
env=dict(os.environ,CARGO_TARGET_DIR=str(T/'work/native-panels-host-target'),CARGO_BUILD_JOBS='4',CARGO_PROFILE_DEV_DEBUG='0',CARGO_INCREMENTAL='0')
with (T/'work/5992-normal-3860faa7e.log').open('wb') as log:
 subprocess.run(['cargo','+stable','xtask','make','host','verify',str(product),'--journey'],cwd=W,env=env,stdout=log,stderr=subprocess.STDOUT,check=True)
packet=W/'target/conduitos/x86_64'; dest=T/'outputs/5992-normal-journey-3860faa7e/x86_64'
assert (packet/'journey-proof.json').is_file() and not dest.exists()
dest.mkdir(parents=True)
for name in ['journey-serial.log','native-shell-paired-linear.json','memory-lantern-native-edit.json','journey-qemu-stderr.log','journey-proof.json','journey-qmp.log','x86_64-product-journey-proof.json']:
 assert (packet/name).is_file(),name
 shutil.copy2(packet/name,dest/name)
shutil.copytree(packet/'journey-frames',dest/'journey-frames')
print('Supported normal journey passed; packet retained',flush=True)
for family in ['panels','pointer']:
 for width,label in [(1280,'wide'),(640,'narrow')]:
  suffix='-cold-birth' if family=='pointer' else ''
  output=T/f'outputs/5992-{family}-{label}-3860faa7e{suffix}'
  assert not output.exists()
  with (T/f'work/5992-{family}-{label}-3860faa7e.log').open('wb') as log:
   subprocess.run(['python3',str(T/'work/5992-pointer-01d9a08e4.py') if family=='pointer' else str(T/'work/6158-runners-e1c6dda63/prove-5990-panels-qemu.py'),str(product),head,str(output),'--width',str(width)],cwd=W,env=env,stdout=log,stderr=subprocess.STDOUT,check=True)
  print(f'{family} {width} passed; packet retained',flush=True)
print('All fresh normal-product emulator runs passed; visual and identity audit remains',flush=True)

for width,label in [(1280,'wide'),(640,'narrow')]:
 output=T/f'outputs/5992-details-{label}-3860faa7e'
 assert not output.exists()
 with (T/f'work/5992-details-{label}-3860faa7e.log').open('wb') as log:
  subprocess.run(['python3',str(T/'work/5992-runners/prove-typography-details.py'),str(product),head,str(output),'--harness',str(T/'work/6158-runners-e1c6dda63/prove-5990-panels-qemu.py'),'--width',str(width)],cwd=W,env=env,stdout=log,stderr=subprocess.STDOUT,check=True)
 print(f'Details {width} passed; packet retained',flush=True)
