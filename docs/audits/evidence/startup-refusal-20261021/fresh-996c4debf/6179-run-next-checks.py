import subprocess,os,json,time,hashlib
from pathlib import Path
root=Path.cwd();cwd=root/'work/6179-recovery';source='996c4debf82bbb2af1f91e68320eef2ae1ce8ea1'
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=cwd,text=True).strip()==source
assert not subprocess.check_output(['git','status','--porcelain'],cwd=cwd)
env=os.environ.copy();env.pop('RUST_MIN_STACK',None);env.update(CARGO_TARGET_DIR='/home/dancxjo/Documents/Codex/2026-10-21/goal-find-the-first-available-github/work/lint-target',CARGO_BUILD_JOBS='2',CARGO_INCREMENTAL='0',CARGO_PROFILE_DEV_DEBUG='0')
base=['cargo','+stable','test','--locked','-p','conduitos','--lib','--features','hosted-tools,native-compositor']
steps=[('startup-tests',base+['product_front_door::startup::tests','--','--nocapture','--test-threads=1']),('lifecycle-tests',base+['product_journey::lifecycle_tests','--','--nocapture','--test-threads=1']),('strict-clippy',['cargo','+stable','clippy','--locked','-p','conduitos','--all-targets','--features','hosted-tools,native-compositor','--','-D','warnings']),('fmt',['cargo','+stable','fmt','--all','--','--check']),('freestanding-check',['cargo','+stable','check','--locked','-p','conduitos','--lib','--target','x86_64-unknown-none','--features','native-compositor'])]
r={'source_commit':source,'proof_boundary':'CPU/frontend/owner fault controls and compile checks; not QEMU failure injection or current-source CI acceptance','status':'running','steps':[]};out=root/'work/6179-next-996c4debf-checks.json'
for name,cmd in steps:
 log=root/f'work/6179-next-996c4debf-{name}.log';assert not log.exists();print('START',name,flush=True);start=time.monotonic()
 with log.open('wb') as f:result=subprocess.run(cmd,cwd=cwd,env=env,stdout=f,stderr=subprocess.STDOUT)
 r['steps'].append({'name':name,'command':cmd,'exit_code':result.returncode,'seconds':time.monotonic()-start,'log':str(log),'sha256':hashlib.sha256(log.read_bytes()).hexdigest()});r['status']='failed' if result.returncode else 'running';out.write_text(json.dumps(r,indent=2)+'\n');print('END',name,result.returncode,flush=True)
 if result.returncode:raise SystemExit(result.returncode)
r['status']='complete';r['source_clean_after']=not subprocess.check_output(['git','status','--porcelain'],cwd=cwd);out.write_text(json.dumps(r,indent=2)+'\n')
