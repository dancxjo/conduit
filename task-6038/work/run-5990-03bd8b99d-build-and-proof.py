from pathlib import Path
import subprocess,os,shutil
T=Path('/home/dancxjo/Documents/Codex/2026-10-19/goal-6038');W=T/'work/native-panels'
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=W,text=True).strip()=='03bd8b99d1018544f7df9bae159e8f416c859689'
assert not subprocess.check_output(['git','status','--porcelain'],cwd=W,text=True).strip()
assert shutil.disk_usage(T).free>30*1024**3
env=dict(os.environ,CARGO_TARGET_DIR=str(T/'work/native-panels-host-target'),CARGO_BUILD_JOBS='4',CARGO_PROFILE_DEV_DEBUG='0',CARGO_INCREMENTAL='0')
with (T/'work/5990-product-03bd8b99d-build.log').open('wb') as log:
 subprocess.run(['cargo','+stable','xtask','make','conduitos','live','x86_64','--locked'],cwd=W,env=env,stdout=log,stderr=subprocess.STDOUT,check=True)
subprocess.run(['python3',str(T/'work/run-5990-03bd8b99d-product-proof.py')],check=True)
