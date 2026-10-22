import subprocess,os,json,time,hashlib,re
from pathlib import Path
root=Path.cwd();cwd=root/'work/6179-recovery';source='856b518cc2f480908d9ee11e994e40b0efa4d3a0'
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=cwd,text=True).strip()==source
assert not subprocess.check_output(['git','status','--porcelain'],cwd=cwd)
prior=json.load(open(root/'work/6179-current-856b518cc-checks.json'));assert prior['status']=='failed' and prior['steps'][-1]['name']=='strict-clippy' and prior['steps'][-1]['exit_code']==101
assert all(step['exit_code']==0 for step in prior['steps'][:2])
env=os.environ.copy();env.pop('RUST_MIN_STACK',None);env.update(CARGO_TARGET_DIR='/home/dancxjo/Documents/Codex/2026-10-21/goal-find-the-first-available-github/work/lint-target',CARGO_BUILD_JOBS='2',CARGO_INCREMENTAL='0',CARGO_PROFILE_DEV_DEBUG='0')
base=['cargo','+stable','test','--locked','-p','conduitos','--lib','--features','hosted-tools,native-compositor']
filters=[('journey-controls','product_journey::tests',6,False),('birth-controls','product_journey::birth::tests',4,False),('forged-failed-show','product_front_door::face_workspace::tests::current_action_rejects_forged_sequence_arguments_and_failed_show',1,True),('replaced-stale-show','product_front_door::face_workspace::tests::replaced_show_and_changed_unobserved_owner_refuse_before_effect',1,True)]
r={'source_commit':source,'status':'running','scope':'Existing named acceptance negatives, not a broad full-suite rerun and not QEMU fault injection','steps':[]};out=root/'work/6179-current-856b518cc-acceptance-controls.json'
for name,filter,count,exact in filters:
 cmd=base+[filter,'--','--nocapture','--test-threads=1']+(['--exact'] if exact else []);log=root/f'work/6179-current-856b518cc-{name}.log';assert not log.exists();print('START',name,flush=True);start=time.monotonic()
 with log.open('wb') as f:result=subprocess.run(cmd,cwd=cwd,env=env,stdout=f,stderr=subprocess.STDOUT)
 match=re.search(r'test result: ok\. (\d+) passed; 0 failed; 0 ignored',log.read_text());valid=result.returncode==0 and match and int(match[1])==count
 r['steps'].append({'name':name,'command':cmd,'exit_code':result.returncode,'expected_passed_tests':count,'actual_passed_tests':int(match[1]) if match else None,'count_verified':bool(valid),'seconds':time.monotonic()-start,'log':str(log),'sha256':hashlib.sha256(log.read_bytes()).hexdigest()});r['status']='running' if valid else 'failed';out.write_text(json.dumps(r,indent=2)+'\n');print('END',name,result.returncode,'count_verified',bool(valid),flush=True)
 if not valid:raise SystemExit(result.returncode or 1)
r['status']='complete';r['source_clean_after']=not subprocess.check_output(['git','status','--porcelain'],cwd=cwd);out.write_text(json.dumps(r,indent=2)+'\n')
