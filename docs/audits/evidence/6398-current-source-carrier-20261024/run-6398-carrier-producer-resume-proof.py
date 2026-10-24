import os,pathlib,subprocess,json,hashlib,time,re,shutil
r=pathlib.Path.cwd();p=pathlib.Path('/tmp/conduit-6398-carrier-refresh-source');source='002b2b44a833b5670e38415ecce3f652687cb372';target=r/'work/6398-carrier-refresh-target';output=r/'work/6398-carrier-refresh';receipt=r/'outputs/6398-carrier-producer-resume-local-proof.json';log=r/'work/6398-carrier-producer-resume.log';assert not receipt.exists()and not output.exists()and not log.exists()
def git(*a):return subprocess.check_output(['git','-C',str(p),*a],text=True).strip()
assert git('rev-parse','HEAD')==source and not git('status','--porcelain');live=json.loads(subprocess.check_output(['gh','pr','view','6398','--repo','dancxjo/conduit','--json','headRefOid,state'],text=True));assert live['headRefOid']==source and live['state']=='OPEN'
# This produces task-owned artifacts only, never changes the owner checkout or PR branch.
protected=[]
for name,count in [('ready-four-combined',138),('ready-four-supported-supplement',104),('ready-four-native',81),('6389-be722',103),('6389-be722-runtime-supported',248),('next-three-focused',182),('next-two-face-proof',92),('6396-linked-validation',85)]:
 old=json.loads((r/f'outputs/{name}-local-proof.json').read_text());assert len(old['executables'])==count;protected+=old['executables']
prior=json.loads((r/'outputs/6398-carrier-producer-local-proof.json').read_text());assert prior['status']=='failed-first-negative-preserved'and len(prior['executables'])==127
protected+=prior['executables']
def sha(f):
 h=hashlib.sha256()
 with pathlib.Path(f).open('rb')as stream:
  for b in iter(lambda:stream.read(1024*1024),b''):h.update(b)
 return h.hexdigest()
for e in protected:assert sha(e['path'])==e['sha256']
free=shutil.disk_usage(r).free;assert free>=9*1024**3,free
env=dict(os.environ,CARGO_TARGET_DIR=str(target),CARGO_BUILD_JOBS='2',CARGO_INCREMENTAL='0',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0');cmd=['cargo','+stable','xtask','prove','journey','native-speech','--output',str(output),'--shared-carrier-only'];x={'source':source,'tree':git('rev-parse','HEAD^{tree}'),'command':cmd,'started':time.time(),'before_free_bytes':free,'environment':{k:env[k]for k in ['CARGO_TARGET_DIR','CARGO_BUILD_JOBS','CARGO_INCREMENTAL','CARGO_PROFILE_DEV_DEBUG','CARGO_PROFILE_TEST_DEBUG']},'protected1033_verified_before':True,'prior127_verified_before':True,'source_materialization_repair':'outputs/6398-source-materialization-repair.json'};print('START',cmd,flush=True)
with log.open('wb')as f:result=subprocess.run(cmd,cwd=p,env=env,stdout=f,stderr=subprocess.STDOUT)
raw=log.read_bytes();counts=list(map(int,re.findall(r'test result: ok\. (\d+) passed; 0 failed; 0 ignored;',raw.decode())));x.update(exit_code=result.returncode,finished=time.time(),log=str(log),log_sha256=sha(log),test_counts=counts,actual_passed=sum(counts),clean_after=not git('status','--porcelain'),status='passed'if result.returncode==0 and sum(counts)==2 else 'failed-first-negative-preserved');x['executables']=[]
for f in target.rglob('*'):
 if f.is_file()and os.access(f,os.X_OK):
  with f.open('rb')as stream:magic=stream.read(4)
  if magic==b'\x7fELF':x['executables'].append({'path':str(f),'bytes':f.stat().st_size,'sha256':sha(f)})
for e in protected:assert sha(e['path'])==e['sha256']
x['protected1033_verified_after']=True
x['prior127_verified_after']=True
x['generated_files']=[{'path':str(f),'bytes':f.stat().st_size,'sha256':sha(f)}for f in output.rglob('*')if f.is_file()]if output.exists()else[]
receipt.write_text(json.dumps(x,indent=2)+'\n');print(x['status'],counts,flush=True)
assert x['status']=='passed'and x['clean_after']
