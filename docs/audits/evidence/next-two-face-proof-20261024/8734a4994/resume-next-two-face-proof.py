import os,subprocess,pathlib,json,hashlib,time,shutil,re
root=pathlib.Path.cwd();checkout=pathlib.Path('/tmp/conduit-next-two-face-proof-source');target=root/'work/next-two-face-proof-target';tmp=root/'work/next-two-face-proof-tmp';tmp.mkdir(exist_ok=True);env=dict(os.environ,CARGO_TARGET_DIR=str(target),TMPDIR=str(tmp),CARGO_BUILD_JOBS='2',CARGO_INCREMENTAL='0',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0')
def git(*a):return subprocess.check_output(['git',*a],cwd=checkout,text=True).strip()
head=git('rev-parse','HEAD');assert head=='8734a4994af36d8b4c28d3b2bd39cc197d65e1ae';tree=git('rev-parse','HEAD^{tree}');assert tree=='d63189bd7311480c447bbc8128fbf09098577fd2';assert not git('status','--porcelain');r={'source':head,'tree':tree,'runtime_tree':git('rev-parse','HEAD:bodies/environment/runtime'),'mask_tree':git('rev-parse','HEAD:plots/seven-segment-mask'),'fore_tree':git('rev-parse','HEAD:architecture/assigned-plan'),'composition_verified':True,'next_two_11_disjoint_path_composition':True,'environment':{k:env[k]for k in ['CARGO_TARGET_DIR','TMPDIR','CARGO_BUILD_JOBS','CARGO_INCREMENTAL','CARGO_PROFILE_DEV_DEBUG','CARGO_PROFILE_TEST_DEBUG']},'steps':[],'started':time.time()};receipt=root/'outputs/next-two-face-proof-local-proof.json';steps=[]
runtime=['--locked','--manifest-path','bodies/environment/runtime/Cargo.toml'];face=['--locked','--manifest-path','bodies/environment/face/Cargo.toml'];features=['--features','face-projection,source-capture']
steps=[('supported-face-publication',['cargo','+stable','xtask','prove','environment','face-publication'],3),('display-packets',['cargo','+stable','test',*runtime,*features,'--test','display_packets'],4),('face-tests',['cargo','+stable','test',*face],9),('runtime-strict',['cargo','+stable','clippy',*runtime,*features,'--all-targets','--','-D','warnings'],0),('face-strict',['cargo','+stable','clippy',*face,'--all-targets','--','-D','warnings'],0),('runtime-no-std',['cargo','+stable','check',*runtime,*features,'--lib','--target','thumbv6m-none-eabi'],0)]
r=json.loads(receipt.read_bytes());assert r['source']==head and r['tree']==tree and r['status']=='stopped-before-next-step-insufficient-headroom' and len(r['steps'])==1 and r['steps'][0]['exit_code']==0
r['resumed_remaining_only_at']=time.time();r['headroom_stop_receipt']='outputs/next-two-face-proof-headroom-stop.json';steps=steps[len(r['steps']):]
protected=[]
for file,n in [('outputs/ready-four-combined-local-proof.json',138),('outputs/ready-four-supported-supplement-local-proof.json',104),('outputs/ready-four-native-local-proof.json',81),('outputs/6389-be722-local-proof.json',103),('outputs/6389-be722-runtime-supported-local-proof.json',248),('outputs/next-three-focused-local-proof.json',182)]:
 old=json.loads((root/file).read_bytes());assert len(old['executables'])==n;protected+=old['executables']
for e in protected:assert hashlib.sha256(pathlib.Path(e['path']).read_bytes()).hexdigest()==e['sha256']
r['protected856_verified_before']=True
for name,cmd,expected in steps:
 free=shutil.disk_usage(tmp).free
 if free<7*1024**3:r.update(status='stopped-before-next-step-insufficient-headroom',next_step=name,available_bytes=free);receipt.write_text(json.dumps(r,indent=2)+'\n');raise SystemExit('Headroom below7GiB; no next command started')
 log=root/f'work/next-two-face-proof-{name}.log';print('START',name,'free',free,cmd,flush=True)
 with log.open('wb')as f:p=subprocess.run(cmd,cwd=checkout,env=env,stdout=f,stderr=subprocess.STDOUT)
 raw=log.read_bytes();counts=list(map(int,re.findall(r'test result: ok\. (\d+) passed; 0 failed; 0 ignored;',raw.decode())));named=[line for line in raw.decode().splitlines()if line.startswith('test ')and line.endswith(' ... ok')];running=re.findall(r'Running[^\n]*\(([^)]+)\)',raw.decode());row={'name':name,'command':cmd,'log':str(log.relative_to(root)),'sha256':hashlib.sha256(raw).hexdigest(),'exit_code':p.returncode,'before_available_bytes':free,'actual_test_counts':counts,'actual_passed':sum(counts),'actual_named_successes':named,'cargo_running_executables':running};r['steps'].append(row);receipt.write_text(json.dumps(r,indent=2)+'\n');print({k:v for k,v in row.items()if k not in ['actual_named_successes','cargo_running_executables','command']},flush=True);
 if p.returncode!=0 or sum(counts)!=expected:
  r.update(status='failed-first-negative-preserved',failed_step=name,expected_tests=expected,clean_after=not git('status','--porcelain'));receipt.write_text(json.dumps(r,indent=2)+'\n');raise SystemExit('Firstnegativepreserved; no subsequentcommand started')
for e in protected:assert hashlib.sha256(pathlib.Path(e['path']).read_bytes()).hexdigest()==e['sha256']
r['protected856_verified_after']=True
r.pop('next_step',None);r.pop('available_bytes',None);r.update(finished=time.time(),clean_after=not git('status','--porcelain'),status='passed');r['executables']=[]
for p in target.rglob('*'):
 if p.is_file()and os.access(p,os.X_OK):
  with p.open('rb')as f:magic=f.read(4)
  if magic==b'\x7fELF':r['executables'].append({'path':str(p),'sha256':hashlib.sha256(p.read_bytes()).hexdigest(),'bytes':p.stat().st_size})
receipt.write_text(json.dumps(r,indent=2)+'\n');assert r['clean_after']and sum(x['actual_passed']for x in r['steps'])==16;print('PASS',len(r['executables']),'ELFs',flush=True)
