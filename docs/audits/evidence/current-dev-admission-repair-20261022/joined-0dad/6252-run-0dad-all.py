import subprocess,sys,os,json,time,re
from pathlib import Path
base=Path('/home/dancxjo/Documents/Codex/2026-10-21/please-finish-and-close-any-conduit');root=base/'work/6252-native-fixtures'
source='0dad64726989f01b79de8f4bb4cd990b2787f040'
subprocess.run([sys.executable,str(base/'work/6252-run-0dad-focused.py')],cwd=base,check=True)
with (base/'work/6252-0dad64726-sdk-errors.log').open('w') as log:
 subprocess.run(['node','--test','targets/browser/sdk/browser-sdk-errors.test.mjs'],cwd=root,stdout=log,stderr=subprocess.STDOUT,check=True)
with (base/'work/6252-0dad64726-resource-tests.log').open('w') as log:
 subprocess.run(['node','--test','proof/browser/workspace-resource-bounds.test.mjs','proof/browser/shared-face-style-package.test.mjs'],cwd=root,stdout=log,stderr=subprocess.STDOUT,check=True)
resource_text=(base/'work/6252-0dad64726-resource-tests.log').read_text()
resource_counts={k:int(re.search(r'^[#ℹ]\s+'+k+r' (\d+)',resource_text,re.M).group(1)) for k in ['tests','pass','fail','cancelled','skipped','todo']}
assert resource_counts['tests']==4 and resource_counts['pass']==4 and all(resource_counts[k]==0 for k in ['fail','cancelled','skipped','todo'])
(base/'work/6252-0dad64726-resource-check.json').write_text(json.dumps(dict(source=source,command=['node','--test','proof/browser/workspace-resource-bounds.test.mjs','proof/browser/shared-face-style-package.test.mjs'],exit_code=0,counts=resource_counts),indent=2)+'\n')
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip()==source
assert not subprocess.check_output(['git','status','--porcelain'],cwd=root,text=True)
env=os.environ.copy();env.pop('RUST_MIN_STACK',None);env.update(CARGO_TARGET_DIR='/home/dancxjo/Documents/Codex/2026-10-21/goal-find-the-first-available-github/work/lint-target',CARGO_BUILD_JOBS='2',CARGO_INCREMENTAL='0',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0')
cmd=['cargo','+stable','test','--locked','-p','conduitos-application-core'];log=base/'work/6252-0dad64726-app-core.log';start=time.time()
with log.open('w') as f:r=subprocess.run(cmd,cwd=root,env=env,stdout=f,stderr=subprocess.STDOUT)
text=log.read_text();counts=[int(x) for x in re.findall(r'^test result: ok\. (\d+) passed; 0 failed;',text,re.M)]
passed=r.returncode==0 and counts and sum(counts)>0 and 'data::codec::tests::reusable_workspace_preserves_exact_wire_and_graph_custody_bytes ... ok' in text and 'PASS exact bitmap grant=' in text and 'SKIP' not in text
(base/'work/6252-0dad64726-app-core-check.json').write_text(json.dumps(dict(source=source,command=cmd,exit_code=r.returncode,pass_verified=bool(passed),harness_cases=sum(counts),fixed_grant_marker='PASS exact bitmap grant=' in text,log=str(log),seconds=time.time()-start),indent=2)+'\n')
print('app-core',r.returncode,bool(passed),sum(counts),flush=True)
if not passed:raise SystemExit(r.returncode or 2)
subprocess.run([sys.executable,str(base/'work/6252-run-0dad-catalog-and-strict.py')],cwd=base,check=True)
