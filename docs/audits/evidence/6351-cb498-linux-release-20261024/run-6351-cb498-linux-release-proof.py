import os,subprocess,pathlib,json,hashlib,time,shutil,re
root=pathlib.Path.cwd();checkout=pathlib.Path('/tmp/conduit-6351-cb498-linux-release-source');target=root/'work/6351-cb498-linux-release-target';tmp=root/'work/6351-cb498-linux-release-tmp';tmp.mkdir(exist_ok=True);env=dict(os.environ,CARGO_TARGET_DIR=str(target),TMPDIR=str(tmp),CARGO_BUILD_JOBS='2',CARGO_INCREMENTAL='0',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0',CARGO_PROFILE_RELEASE_DEBUG='0')
def git(*a):return subprocess.check_output(['git',*a],cwd=checkout,text=True).strip()
head=git('rev-parse','HEAD');assert head=='cb4983f58b3f785734c166243514cbb848ea2f9a';tree=git('rev-parse','HEAD^{tree}');assert tree=='033712120b7c5ad812a18969e98602f3edf17d24';assert not git('status','--porcelain');r={'source':head,'tree':tree,'runtime_tree':git('rev-parse','HEAD:bodies/environment/runtime'),'mask_tree':git('rev-parse','HEAD:plots/seven-segment-mask'),'fore_tree':git('rev-parse','HEAD:architecture/assigned-plan'),'composition_verified':True,'exact_owner_head_no_composition':True,'environment':{k:env[k]for k in ['CARGO_TARGET_DIR','TMPDIR','CARGO_BUILD_JOBS','CARGO_INCREMENTAL','CARGO_PROFILE_DEV_DEBUG','CARGO_PROFILE_TEST_DEBUG','CARGO_PROFILE_RELEASE_DEBUG']},'steps':[],'started':time.time()};receipt=root/'outputs/6351-cb498-linux-release-local-proof.json';steps=[('supported-linux-release',['cargo','+stable','xtask','make','host','release','--platform','linux','--output',str(root/'work/6351-cb498-linux-release')],0)]
protected=[]
for file,n in [('outputs/ready-four-combined-local-proof.json',138),('outputs/ready-four-supported-supplement-local-proof.json',104),('outputs/ready-four-native-local-proof.json',81),('outputs/6389-be722-local-proof.json',103),('outputs/6389-be722-runtime-supported-local-proof.json',248),('outputs/next-three-focused-local-proof.json',182),('outputs/next-two-face-proof-local-proof.json',92),('outputs/6396-linked-validation-local-proof.json',85)]:
 old=json.loads((root/file).read_bytes());assert len(old['executables'])==n;protected+=old['executables']
for e in protected:assert hashlib.sha256(pathlib.Path(e['path']).read_bytes()).hexdigest()==e['sha256']
r['protected1033_verified_before']=True
for name,cmd,expected in steps:
 free=shutil.disk_usage(tmp).free
 if free<7*1024**3:r.update(status='stopped-before-next-step-insufficient-headroom',next_step=name,available_bytes=free);receipt.write_text(json.dumps(r,indent=2)+'\n');raise SystemExit('Headroom below7GiB; no next command started')
 log=root/f'work/6351-cb498-linux-release-{name}.log';print('START',name,'free',free,cmd,flush=True)
 with log.open('wb')as f:p=subprocess.run(cmd,cwd=checkout,env=env,stdout=f,stderr=subprocess.STDOUT)
 raw=log.read_bytes();counts=list(map(int,re.findall(r'test result: ok\. (\d+) passed; 0 failed; 0 ignored;',raw.decode())));named=[line for line in raw.decode().splitlines()if line.startswith('test ')and line.endswith(' ... ok')];running=re.findall(r'Running[^\n]*\(([^)]+)\)',raw.decode());row={'name':name,'command':cmd,'log':str(log.relative_to(root)),'sha256':hashlib.sha256(raw).hexdigest(),'exit_code':p.returncode,'before_available_bytes':free,'actual_test_counts':counts,'actual_passed':sum(counts),'actual_named_successes':named,'cargo_running_executables':running};r['steps'].append(row);receipt.write_text(json.dumps(r,indent=2)+'\n');print({k:v for k,v in row.items()if k not in ['actual_named_successes','cargo_running_executables','command']},flush=True);
 if p.returncode!=0 or sum(counts)!=expected:
  r.update(status='failed-first-negative-preserved',failed_step=name,expected_tests=expected,clean_after=not git('status','--porcelain'));receipt.write_text(json.dumps(r,indent=2)+'\n');raise SystemExit('Firstnegativepreserved; no subsequentcommand started')
for e in protected:assert hashlib.sha256(pathlib.Path(e['path']).read_bytes()).hexdigest()==e['sha256']
r['protected1033_verified_after']=True
r.update(finished=time.time(),clean_after=not git('status','--porcelain'),status='passed');r['executables']=[]
for p in target.rglob('*'):
 if p.is_file()and os.access(p,os.X_OK):
  with p.open('rb')as f:magic=f.read(4)
  if magic==b'\x7fELF':r['executables'].append({'path':str(p),'sha256':hashlib.sha256(p.read_bytes()).hexdigest(),'bytes':p.stat().st_size})
receipt.write_text(json.dumps(r,indent=2)+'\n');assert r['clean_after']and sum(x['actual_passed']for x in r['steps'])==0;print('PASS',len(r['executables']),'ELFs',flush=True)
