from pathlib import Path
import subprocess,os,json,time
T=Path('/home/dancxjo/Documents/Codex/2026-10-19/goal-6038');W=T/'work/native-panels';H='e29922753a2fca04145551bbb6ae00504fed76a9'
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=W,text=True).strip()==H
assert not subprocess.check_output(['git','status','--porcelain'],cwd=W,text=True).strip()
env=dict(os.environ,CARGO_TARGET_DIR=str(T/'work/native-panels-host-target'),CARGO_BUILD_JOBS='4',CARGO_PROFILE_DEV_DEBUG='0',CARGO_INCREMENTAL='0',CARGO_PROFILE_TEST_DEBUG='0',CARGO_PROFILE_TEST_OPT_LEVEL='1',CARGO_PROFILE_TEST_DEBUG_ASSERTIONS='true',CARGO_PROFILE_TEST_OVERFLOW_CHECKS='true')
steps=[('6055-e29922753-preflight',['cargo','+stable','xtask','ci','pipeline','preflight','80bf55640ae789952bd98f54258b0c02d0d5bd5d']),('5990-e29922753-strict-clippy',['cargo','+stable','clippy','-p','conduitos','--lib','--tests','--locked','--features','hosted-tools,native-compositor','--','-D','warnings']),('5990-e29922753-native-face-tests',['cargo','+stable','test','-p','conduitos','--lib','--locked','--features','hosted-tools,native-compositor','native_face','--','--test-threads=1']),('5990-product-e29922753-build',['cargo','+stable','xtask','make','conduitos','live','x86_64','--locked']),('5990-e29922753-five-runs',['python3',str(T/'work/run-5990-e29922753-product-proof.py')]),('5990-e29922753-consolidation',['python3',str(T/'work/consolidate-5990-e29922753.py')])]
results=[]
for name,argv in steps:
 assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=W,text=True).strip()==H
 assert not subprocess.check_output(['git','status','--porcelain'],cwd=W,text=True).strip()
 with (T/'work'/f'{name}.log').open('wb') as log:
  start=time.time();p=subprocess.run(argv,cwd=W,env=env,stdout=log,stderr=subprocess.STDOUT)
 results.append(dict(name=name,argv=argv,status=p.returncode,seconds=time.time()-start))
 (T/'work/5990-e29922753-sequence-status.json').write_text(json.dumps(results,indent=2)+'\n')
 print(name,p.returncode,flush=True)
 if p.returncode:raise SystemExit(p.returncode)
