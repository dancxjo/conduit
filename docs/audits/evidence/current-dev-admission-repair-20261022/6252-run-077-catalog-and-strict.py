import os,json,subprocess,time,re,sys
from pathlib import Path
base=Path('/home/dancxjo/Documents/Codex/2026-10-21/please-finish-and-close-any-conduit');root=base/'work/6252-native-fixtures'
source='077a2a535b4ec30d1f09ec6c2884c22bbffd1c57'
prior=json.loads((base/'work/6252-077a2a535-focused-checks.json').read_text());assert prior['source']==source and len(prior['checks'])==9 and all(x['pass_verified'] for x in prior['checks'])
sdk=(base/'work/6252-077a2a535-sdk-errors.log').read_text()
counts={k:int(re.search(r'^[#ℹ]\s+'+k+r' (\d+)',sdk,re.M).group(1)) for k in ['tests','pass','fail','cancelled','skipped','todo']}
assert counts['tests']==6 and counts['pass']==6 and all(counts[k]==0 for k in ['fail','cancelled','skipped','todo'])
(base/'work/6252-077a2a535-sdk-check.json').write_text(json.dumps(dict(source=source,command=['node','--test','targets/browser/sdk/browser-sdk-errors.test.mjs'],exit_code=0,counts=counts,node_version=subprocess.check_output(['node','--version'],text=True).strip()),indent=2)+'\n')
env=os.environ.copy();env.pop('RUST_MIN_STACK',None);env.update(CARGO_TARGET_DIR='/home/dancxjo/Documents/Codex/2026-10-21/goal-find-the-first-available-github/work/lint-target',CARGO_BUILD_JOBS='2',CARGO_INCREMENTAL='0',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0')
out=base/'work/6252-077a2a535-catalog.json'
commands=[('catalog',['cargo','+stable','xtask','check','plots','bundle-workspace-catalog','--output',str(out)],None),('catalog-refusals',['cargo','+stable','test','--locked','-p','xtask','workspace_patchbay_catalog','--','--nocapture'],2),('catalog-regression',['cargo','+stable','test','--locked','-p','xtask','workspace_catalog_preserves_memory_lantern_semantic_editor_requirement','--','--nocapture'],1)]
rows=[]
for label,cmd,expected in commands:
 assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip()==source
 assert not subprocess.check_output(['git','status','--porcelain'],cwd=root,text=True)
 log=base/f'work/6252-077a2a535-{label}.log';start=time.time()
 with log.open('w') as f:r=subprocess.run(cmd,cwd=root,env=env,stdout=f,stderr=subprocess.STDOUT)
 passed=r.returncode==0 and (expected is None or f'{expected} passed; 0 failed' in log.read_text())
 if passed and expected is None:
  v=json.loads(out.read_text());p=next(x for x in v['plots'] if x['slug']=='patchbay')
  assert len(v['plots'])==94 and p['source']==(root/'plots/patchbay/main.conduit').read_text() and p['required_kinds']==['patchbay/accept-interaction']
 rows.append(dict(label=label,command=cmd,exit_code=r.returncode,expected_cases=expected,pass_verified=passed,log=str(log),seconds=time.time()-start))
 (base/'work/6252-077a2a535-catalog-checks.json').write_text(json.dumps(dict(source=source,checks=rows),indent=2)+'\n');print(label,r.returncode,passed,flush=True)
 if not passed:raise SystemExit(r.returncode or 2)
assert not subprocess.check_output(['git','status','--porcelain'],cwd=root,text=True)
raise SystemExit(subprocess.run([sys.executable,str(base/'work/6252-run-077-strict.py')],cwd=base).returncode)
