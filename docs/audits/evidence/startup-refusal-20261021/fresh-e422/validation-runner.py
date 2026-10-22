import hashlib,json,os,re,subprocess,time
from pathlib import Path
root=Path('/home/dancxjo/Documents/Codex/2026-10-21/please-finish-and-close-any-conduit')
env=os.environ.copy();env.update(CARGO_TARGET_DIR='/home/dancxjo/Documents/Codex/2026-10-21/goal-find-the-first-available-github/work/lint-target',CARGO_BUILD_JOBS='4',CARGO_INCREMENTAL='0',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0')
assert not env.get('RUST_MIN_STACK')
def git(path,*args):return subprocess.check_output(['git','-C',str(path),*args],text=True).strip()
def run_suite(name,path,sha,steps):
 assert git(path,'rev-parse','HEAD')==sha
 assert not git(path,'status','--porcelain')
 receipt={'source_commit':sha,'cwd':str(path),'source_clean_before':True,'steps':[],'scope':'hosted checks only; no QEMU, CI or acceptance claim'}
 out=root/'work'/f'{name}-fresh-validation.json'
 for label,cmd,count in steps:
  log=root/'work'/f'{name}-{label}.log';start=time.monotonic()
  print('START',name,label,flush=True)
  with log.open('wb') as f: code=subprocess.call(cmd,cwd=path,env=env,stdout=f,stderr=subprocess.STDOUT)
  data=log.read_bytes();entry={'name':label,'command':cmd,'exit_code':code,'seconds':time.monotonic()-start,'log':str(log),'bytes':len(data),'sha256':hashlib.sha256(data).hexdigest()}
  if count is not None:
   actual=sum(int(n) for n in re.findall(rb'test result: ok\. (\d+) passed;',data));entry.update(expected_passed_tests=count,actual_passed_tests=actual,count_verified=actual==count)
  receipt['steps'].append(entry);receipt['source_clean_after']=not git(path,'status','--porcelain');receipt['source_head_after']=git(path,'rev-parse','HEAD')
  receipt['status']='failed' if code or (count is not None and actual!=count) else 'in-progress'
  out.write_text(json.dumps(receipt,indent=2)+'\n');print('END',name,label,code,flush=True)
  if receipt['status']=='failed':return False
 receipt['status']='passed';out.write_text(json.dumps(receipt,indent=2)+'\n');return True
if not run_suite('6063-f79d',root/'work/6063-canonical-export','f79d280e7c56f8bd5dd108b6102e84816026b782',[('canonical-export',['cargo','+stable','xtask','check','canonical-export'],20)]):raise SystemExit(1)
base=['cargo','+stable','test','--locked','-p','conduitos','--lib','--features','hosted-tools,native-compositor']
steps=[]
for label,filter,count,exact in [
 ('stale-show','product_front_door::face_workspace::tests::replaced_show_and_changed_unobserved_owner_refuse_before_effect',1,True),
 ('startup','product_front_door::startup::tests',6,False),
 ('lifecycle','product_journey::lifecycle_tests',8,False),
 ('journey','product_journey::tests',6,False),
 ('birth','product_journey::birth::tests',4,False),
 ('forged-failed-show','product_front_door::face_workspace::tests::current_action_rejects_forged_sequence_arguments_and_failed_show',1,True)]:
 steps.append((label,base+[filter,'--','--nocapture','--test-threads=1']+(['--exact'] if exact else []),count))
steps.extend([('strict-clippy',['cargo','+stable','clippy','--locked','-p','conduitos','--all-targets','--features','hosted-tools,native-compositor','--','-D','warnings'],None),('fmt',['cargo','+stable','fmt','--all','--','--check'],None)])
if not run_suite('6179-e422',root/'work/6179-recovery','e422ea7323a4804a5315df4e4d1f6c18fbf9a2c9',steps):raise SystemExit(1)
