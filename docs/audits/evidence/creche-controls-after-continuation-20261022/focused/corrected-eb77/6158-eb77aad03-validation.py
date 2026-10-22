import os,pathlib,subprocess,json,hashlib,re
root=pathlib.Path('/home/dancxjo/Documents/Codex/2026-10-21/please-finish-and-close-any-conduit');cwd=root/'work/6158-bounded-captures';source='eb77aad03dae3654709e8004f617d48550e95a9d'
env=os.environ.copy();env.pop('RUST_MIN_STACK',None);env.update(CARGO_TARGET_DIR='/home/dancxjo/Documents/Codex/2026-10-21/goal-find-the-first-available-github/work/lint-target',CARGO_BUILD_JOBS='4',CARGO_INCREMENTAL='0',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0')
def git(*args):return subprocess.check_output(['git','-C',str(cwd),*args],text=True).strip()
assert git('rev-parse','HEAD')==source and not git('status','--porcelain')
host=['cargo','+stable','test','--locked','-p','xtask'];cpu=['cargo','+stable','test','--locked','-p','conduitos','--lib','--features','hosted-tools,native-compositor']
steps=[]
for name,filt,count in [('naming-acceptance','naming_acceptance_tests',2),('field-clusters','native_face_scene::layout::field_cluster_tests',6),('actual-creche','native_face_scene::layout::panels::tests::creche::',4),('creche-navigation','native_face_scene::creche_navigation_tests',5)]:steps.append((name,cpu+[filt,'--','--nocapture','--test-threads=1'],count))
steps += [('integration-naming',['cargo','+stable','test','--locked','-p','conduitos','--test','native_face_scene','--features','hosted-tools,native-compositor','naming_acceptance_tests','--','--nocapture','--test-threads=1'],2),('conduitos-clippy',['cargo','+stable','clippy','--locked','-p','conduitos','--all-targets','--features','hosted-tools,native-compositor','--','-D','warnings'],None),('fmt',['cargo','+stable','fmt','--all','--','--check'],None)]
receipt={'source_commit':source,'cwd':str(cwd),'source_clean_before':True,'RUST_MIN_STACK':'unset','jobs':4,'steps':[],'qemu':'unstarted'}
for name,cmd,count in steps:
 assert git('rev-parse','HEAD')==source and not git('status','--porcelain')
 log=root/'work'/f'6158-eb77aad03-{name}.log';print('START',name,flush=True)
 with log.open('wb') as out:r=subprocess.run(cmd,cwd=cwd,env=env,stdout=out,stderr=subprocess.STDOUT)
 data=log.read_bytes();entry={'name':name,'command':cmd,'exit_code':r.returncode,'sha256':hashlib.sha256(data).hexdigest(),'log':str(log),'bytes':len(data)}
 actual=sum(int(n) for n in re.findall(rb'test result: ok\. (\d+) passed;',data))
 if count is not None:entry.update(expected_tests=count,actual_tests=actual)
 failed=r.returncode!=0 or(count is not None and actual!=count)
 receipt['steps'].append(entry);receipt.update(status='failed' if failed else 'in-progress',source_clean_after=not git('status','--porcelain'),source_head_after=git('rev-parse','HEAD'))
 (root/'work/6158-eb77aad03-validation.json').write_text(json.dumps(receipt,indent=2)+'\n');print('END',name,r.returncode,actual,flush=True)
 if failed:raise SystemExit(r.returncode or 1)
receipt['status']='passed';(root/'work/6158-eb77aad03-validation.json').write_text(json.dumps(receipt,indent=2)+'\n')
