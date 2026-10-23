import pathlib,subprocess,json,os,time,shutil,signal,hashlib
R=pathlib.Path(__file__).parent; W=R/'6176-current-delivery'; O=R/'6176-current-proof';O.mkdir(exist_ok=True)
def sha(p):
 h=hashlib.sha256()
 with p.open('rb') as f:
  for b in iter(lambda:f.read(1048576),b''):h.update(b)
 return h.hexdigest()
def git(*args):return subprocess.check_output(['git','-C',str(W),*args],text=True).strip()
source=git('rev-parse','HEAD');assert source=='74bf191d84c1c15bad1e6f36479ac30294f0bd47' and not git('status','--porcelain=v1','--untracked-files=all')
env=os.environ.copy();env.update(CARGO_TARGET_DIR=str(R/'domain-6183-build-target'),CARGO_BUILD_JOBS='2',CARGO_INCREMENTAL='0',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0',CONDUIT_CONDUITOS_TARGET_ROOT=str(O/'native'))
steps=[('focused-contracts',['cargo','+stable','xtask','check','privilege-inventory']),('strict-clippy',['cargo','+stable','clippy','--locked','-p','xtask','--all-targets','--','-D','warnings']),('normal-product-build',['cargo','+stable','xtask','make','host','build','targets/conduitos/profiles/conduitos-native.host.conduit','--output',str(O/'product'),'--locked']),('positive-verifier',['cargo','+stable','xtask','make','conduitos','verify-privilege-inventory','--build-dir',str(O/'product'),'--inventory-dir',str(O/'native/x86_64'),'--output',str(O/'verified-inventory.json')])]
receipt={'source':source,'runner_pid':os.getpid(),'status':'running','environment':{k:env[k] for k in ['CARGO_TARGET_DIR','CARGO_BUILD_JOBS','CARGO_INCREMENTAL','CARGO_PROFILE_DEV_DEBUG','CARGO_PROFILE_TEST_DEBUG','CONDUIT_CONDUITOS_TARGET_ROOT']},'disk_floor_bytes':6*1024**3,'preflight_free_bytes':shutil.disk_usage(O).free,'steps':[],'proof_class':'source/dependency/linked-image production inventory; no QEMU or isolation execution claim'}
def save():(O/'run.json').write_text(json.dumps(receipt,indent=2)+'\n')
save();print('runner_pid',os.getpid(),flush=True)
for name,cmd in steps:
 log=O/(name+'.log');assert not log.exists();start=time.time();entry={'name':name,'command':cmd,'started':start,'log':str(log),'attempt':1,'status':'running'};receipt['steps'].append(entry)
 with log.open('wb') as f:
  p=subprocess.Popen(cmd,cwd=W,env=env,stdout=f,stderr=subprocess.STDOUT,start_new_session=True);entry['pid']=p.pid;save();print('START',name,'pid',p.pid,flush=True)
  while p.poll() is None:
   free=shutil.disk_usage(O).free
   if free<receipt['disk_floor_bytes']:
    entry['disk_floor_refusal']={'free_bytes':free,'time':time.time()};os.killpg(p.pid,signal.SIGTERM);p.wait();break
   time.sleep(2)
  entry.update(exit_code=p.returncode,seconds=time.time()-start,status='passed' if p.returncode==0 else 'failed')
 entry['sha256']=sha(log);save();print('END',name,'exit',p.returncode,'seconds',round(entry['seconds'],2),flush=True)
 if p.returncode:
  receipt['status']='stopped-first-failure';save();raise SystemExit(p.returncode)
receipt['status']='positive-complete-negative-pending';receipt['clean_after']=not git('status','--porcelain=v1','--untracked-files=all');save();print('POSITIVE_COMPLETE_NEGATIVE_PENDING',flush=True)
