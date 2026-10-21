"""Serial supported product build/proof at the frozen reconciled source."""
from pathlib import Path
import hashlib,json,os,subprocess,time
T=Path('/home/dancxjo/Documents/Codex/2026-10-19/goal-6038');W=T/'work/selected-look-current-owner';O=T/'outputs/5628-current-owner-products';H='db1f41fcc4fe2dea2fe9eaedf799855661542eab'
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=W,text=True).strip()==H
assert not subprocess.check_output(['git','status','--porcelain'],cwd=W,text=True).strip()
O.mkdir();env=dict(os.environ,CARGO_TARGET_DIR=str(T/'work/native-panels-host-target'),CARGO_BUILD_JOBS='4',CARGO_INCREMENTAL='0',CARGO_PROFILE_DEV_DEBUG='0');results=[]
steps=[('normal-product',['cargo','+stable','xtask','make','conduitos','live','x86_64']),('canonical-journey',['cargo','+stable','xtask','make','host','verify',str(W/'target/conduitos/live/x86_64-pc'),'--journey']),('linux-release',['cargo','+stable','xtask','make','host','release','--platform','linux','--output','target/5628-linux-release']),('handbook',['cargo','+stable','xtask','make','body','static','--application','targets/browser/handbook/handbook.application.template.json','--handbook','--output','target/5628-handbook'])]
for name,args in steps:
 assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=W,text=True).strip()==H
 assert not subprocess.check_output(['git','status','--porcelain'],cwd=W,text=True).strip()
 p=O/(name+'.log');start=time.time();(O/'status.json').write_text(json.dumps(dict(source=H,status='running',active=name,steps=results),indent=2)+'\n')
 with p.open('xb') as out:r=subprocess.run(args,cwd=W,env=env,stdout=out,stderr=subprocess.STDOUT)
 results.append(dict(name=name,args=args,exit_code=r.returncode,seconds=time.time()-start,log_sha256=hashlib.sha256(p.read_bytes()).hexdigest()));(O/'status.json').write_text(json.dumps(dict(source=H,status='failed' if r.returncode else 'running',active=None,steps=results),indent=2)+'\n');print(name,r.returncode,flush=True)
 if r.returncode:raise SystemExit(r.returncode)
(O/'status.json').write_text(json.dumps(dict(source=H,status='passed',steps=results),indent=2)+'\n')
