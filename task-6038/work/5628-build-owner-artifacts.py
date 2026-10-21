from pathlib import Path
import subprocess,os,json,time,hashlib
T=Path('/home/dancxjo/Documents/Codex/2026-10-19/goal-6038');W=T/'work/selected-look-reconciled';O=T/'outputs/5628-reconciled-253dcb985';H='253dcb985eb8890ddf873d672d3a282f2366e6db'
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=W,text=True).strip()==H
assert not subprocess.check_output(['git','status','--porcelain'],cwd=W,text=True).strip()
env=dict(os.environ,CARGO_TARGET_DIR=str(T/'work/native-panels-host-target'),CARGO_BUILD_JOBS='4',CARGO_INCREMENTAL='0',CARGO_PROFILE_DEV_DEBUG='0')
steps=[('linux-release',['cargo','+stable','xtask','make','host','release','--platform','linux','--output','target/5628-linux-release']),('handbook',['cargo','+stable','xtask','make','body','static','--application','targets/browser/handbook/handbook.application.template.json','--handbook','--output','target/5628-handbook'])]
results=[]
for name,args in steps:
 start=time.time();log=O/(name+'-build.log');assert not log.exists()
 with log.open('wb') as f:r=subprocess.run(args,cwd=W,env=env,stdout=f,stderr=subprocess.STDOUT)
 results.append(dict(name=name,args=args,exit_code=r.returncode,seconds=time.time()-start,log_sha256=hashlib.sha256(log.read_bytes()).hexdigest()))
 (O/'owner-artifact-build-status.json').write_text(json.dumps(dict(source=H,steps=results),indent=2)+'\n')
 print(name,r.returncode,flush=True)
 if r.returncode:raise SystemExit(r.returncode)
