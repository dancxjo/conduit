import os,json,subprocess,time
from pathlib import Path
base=Path('/home/dancxjo/Documents/Codex/2026-10-21/please-finish-and-close-any-conduit')
root=base/'work/6252-native-fixtures'
source='0dad64726989f01b79de8f4bb4cd990b2787f040'
prior=json.loads((base/'work/6252-0dad64726-focused-checks.json').read_text())
assert prior['source']==source and len(prior['checks'])==9 and all(x['pass_verified'] for x in prior['checks'])
env=os.environ.copy();env.pop('RUST_MIN_STACK',None)
env.update(CARGO_TARGET_DIR='/home/dancxjo/Documents/Codex/2026-10-21/goal-find-the-first-available-github/work/lint-target',CARGO_BUILD_JOBS='2',CARGO_INCREMENTAL='0',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0')
commands=[('fmt',['cargo','+stable','fmt','--all','--check']),('strict-packages',['cargo','+stable','clippy','--locked','-p','conduitos','-p','conduit-browser-runtime','-p','xtask','--all-targets','--features','conduitos/native-compositor','--','-D','warnings'])]
rows=[]
for label,cmd in commands:
 assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip()==source
 assert not subprocess.check_output(['git','status','--porcelain'],cwd=root,text=True)
 log=base/f'work/6252-0dad64726-{label}.log';start=time.time()
 with log.open('w') as f:r=subprocess.run(cmd,cwd=root,env=env,stdout=f,stderr=subprocess.STDOUT)
 rows.append(dict(label=label,command=cmd,exit_code=r.returncode,log=str(log),seconds=time.time()-start))
 (base/'work/6252-0dad64726-strict-checks.json').write_text(json.dumps(dict(source=source,checks=rows),indent=2)+'\n')
 print(label,r.returncode,flush=True)
 if r.returncode:raise SystemExit(r.returncode)
assert not subprocess.check_output(['git','status','--porcelain'],cwd=root,text=True)
