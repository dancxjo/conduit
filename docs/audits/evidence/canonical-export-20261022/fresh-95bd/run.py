from pathlib import Path
import subprocess,os,time,json,hashlib,re,datetime
root=Path('/home/dancxjo/Documents/Codex/2026-10-21/please-finish-and-close-any-conduit');cwd=root/'work/6063-canonical-export';source='95bd7a776ebe67a597a69dc88b758196b86ce3fa'
def git(*a):return subprocess.check_output(['git',*a],cwd=cwd,text=True).strip()
assert git('rev-parse','HEAD')==source and not git('status','--porcelain')
env=os.environ.copy();env.pop('RUST_MIN_STACK',None);settings={'CARGO_TARGET_DIR':'/home/dancxjo/Documents/Codex/2026-10-21/goal-find-the-first-available-github/work/lint-target','CARGO_BUILD_JOBS':'4','CARGO_INCREMENTAL':'0','CARGO_PROFILE_DEV_DEBUG':'0','CARGO_PROFILE_TEST_DEBUG':'0'};env.update(settings)
cmd=['cargo','+stable','xtask','check','canonical-export'];log=root/'work/6063-95bd-canonical-export.log';t=time.monotonic();start=datetime.datetime.now(datetime.timezone.utc).isoformat()
with log.open('wb') as f:code=subprocess.call(cmd,cwd=cwd,env=env,stdout=f,stderr=subprocess.STDOUT)
b=log.read_bytes();counts=[int(n) for n in re.findall(rb'test result: ok\. (\d+) passed;',b)]
receipt={'schema':'conduit.audit/canonical-export-check@1','source_commit':source,'cwd':str(cwd),'command':cmd,'environment':settings,'RUST_MIN_STACK':None,'started_utc':start,'seconds':time.monotonic()-t,'exit_code':code,'status':'passed' if code==0 and counts==[6,14] else 'failed','passed_test_groups':counts,'expected_test_groups':[6,14],'tests_passed':sum(counts),'strict_steps_expected':2,'strict_steps_complete':code==0,'source_head_after':git('rev-parse','HEAD'),'source_clean_after':not git('status','--porcelain'),'log':str(log),'raw_bytes':len(b),'raw_sha256':hashlib.sha256(b).hexdigest(),'scope':'Hosted canonical export behavior and suite strict checks only; no native/QEMU, PR or CI claim.'}
(root/'outputs/6063-95bd-canonical-export-check.json').write_text(json.dumps(receipt,indent=2)+'\n');print(json.dumps(receipt,indent=2));raise SystemExit(0 if receipt['status']=='passed' else 1)
