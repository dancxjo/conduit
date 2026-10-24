import os,subprocess,pathlib,json,hashlib,time,shutil,re
root=pathlib.Path.cwd();checkout=root/'work/6382-6ab-proof';target=root/'work/6382-6ab-target';tmp=root/'work/6382-6ab-tmp';tmp.mkdir(exist_ok=True);env=dict(os.environ,CARGO_TARGET_DIR=str(target),TMPDIR=str(tmp),CARGO_BUILD_JOBS='2',CARGO_INCREMENTAL='0',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0')
def git(*a):return subprocess.check_output(['git',*a],cwd=checkout,text=True).strip()
head=git('rev-parse','HEAD');assert head=='6ab212c0016048791bcd014f3931f6658c1fe712';assert not git('status','--porcelain');assert shutil.disk_usage(root).free>=7*1024**3
r={'source':head,'tree':git('rev-parse','HEAD^{tree}'),'base':'91430c2d2af0bb89525916f81efcf5f74bb4a2db','environment':{k:env[k]for k in ['CARGO_TARGET_DIR','TMPDIR','CARGO_BUILD_JOBS','CARGO_INCREMENTAL','CARGO_PROFILE_DEV_DEBUG','CARGO_PROFILE_TEST_DEBUG']},'steps':[],'started':time.time()};receipt=root/'outputs/6382-6ab-local-proof.json'
for name,cmd in [('mask-tests',['cargo','+stable','test','--locked','--manifest-path','plots/seven-segment-mask/Cargo.toml']),('strict-lint',['cargo','+stable','clippy','--locked','--manifest-path','plots/seven-segment-mask/Cargo.toml','--all-targets','--','-D','warnings'])]:
 log=root/f'work/6382-6ab-{name}.log';print('START',name,cmd,flush=True)
 with log.open('wb')as f:p=subprocess.run(cmd,cwd=checkout,env=env,stdout=f,stderr=subprocess.STDOUT)
 raw=log.read_bytes();row={'name':name,'command':cmd,'log':str(log.relative_to(root)),'sha256':hashlib.sha256(raw).hexdigest(),'exit_code':p.returncode,'actual_passed':sum(map(int,re.findall(r'test result: ok\. (\d+) passed; 0 failed; 0 ignored;',raw.decode())))};r['steps'].append(row);receipt.write_text(json.dumps(r,indent=2)+'\n');print(row,flush=True);assert p.returncode==0
r.update(finished=time.time(),clean_after=not git('status','--porcelain'));r['executables']=[]
for p in target.rglob('*'):
 if p.is_file()and os.access(p,os.X_OK):
  with p.open('rb')as f:magic=f.read(4)
  if magic==b'\x7fELF':r['executables'].append({'path':str(p.relative_to(root)),'sha256':hashlib.sha256(p.read_bytes()).hexdigest(),'bytes':p.stat().st_size})
receipt.write_text(json.dumps(r,indent=2)+'\n');assert r['clean_after']and r['steps'][0]['actual_passed']==13;print('PASS',flush=True)
