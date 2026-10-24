import os,subprocess,pathlib,json,hashlib,time,shutil,re
root=pathlib.Path.cwd();checkout=root/'work/6313-current-dev-c';target=root/'work/6313-current-dev-c-target';tmp=root/'work/6313-current-dev-c-tmp';tmp.mkdir(exist_ok=True);env=dict(os.environ,CARGO_TARGET_DIR=str(target),TMPDIR=str(tmp),CARGO_BUILD_JOBS='2',CARGO_INCREMENTAL='0',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0')
def git(*a):return subprocess.check_output(['git',*a],cwd=checkout,text=True).strip()
head=git('rev-parse','HEAD');assert head=='40e63b6a7a79bb5691f98f0706396630bf168047';assert not git('status','--porcelain');assert shutil.disk_usage(root).free>=7*1024**3
paths=['bodies/environment/runtime/tests/observation_kernel.rs','bodies/environment/runtime/tests/support/assignment.rs'];assert git('diff','--name-only','91430c2d2af0bb89525916f81efcf5f74bb4a2db','HEAD').splitlines()==paths
for p in paths:assert git('rev-parse','HEAD:'+p)==git('rev-parse','c4914d0043cea604b66492ffada27d4d05d7533e:'+p)
assert git('rev-parse','HEAD:bodies/environment/runtime/src')==git('rev-parse','91430c2d2af0bb89525916f81efcf5f74bb4a2db:bodies/environment/runtime/src')
r={'source':head,'tree':git('rev-parse','HEAD^{tree}'),'base':'91430c2d2af0bb89525916f81efcf5f74bb4a2db','old_source':'c4914d0043cea604b66492ffada27d4d05d7533e','delta_paths':paths,'bridge_blobs_exact_old':True,'production_runtime_tree_exact_captured_dev':True,'environment':{k:env[k]for k in ['CARGO_TARGET_DIR','TMPDIR','CARGO_BUILD_JOBS','CARGO_INCREMENTAL','CARGO_PROFILE_DEV_DEBUG','CARGO_PROFILE_TEST_DEBUG']},'steps':[],'started':time.time()};receipt=root/'outputs/6313-current-dev-c-proof.json'
for name,cmd in [('runtime',['cargo','+stable','test','--locked','--manifest-path','bodies/environment/runtime/Cargo.toml']),('strict-lint',['cargo','+stable','clippy','--locked','--manifest-path','bodies/environment/runtime/Cargo.toml','--all-targets','--','-D','warnings'])]:
 log=root/f'work/6313-current-dev-c-{name}.log';print('START',name,cmd,flush=True)
 with log.open('wb')as f:p=subprocess.run(cmd,cwd=checkout,env=env,stdout=f,stderr=subprocess.STDOUT)
 raw=log.read_bytes();row={'name':name,'command':cmd,'log':str(log.relative_to(root)),'sha256':hashlib.sha256(raw).hexdigest(),'exit_code':p.returncode,'actual_passed':sum(map(int,re.findall(r'test result: ok\. (\d+) passed; 0 failed; 0 ignored;',raw.decode())))};r['steps'].append(row);receipt.write_text(json.dumps(r,indent=2)+'\n');print(row,flush=True);assert p.returncode==0
r.update(finished=time.time(),clean_after=not git('status','--porcelain'));r['executables']=[]
for p in target.rglob('*'):
 if p.is_file()and os.access(p,os.X_OK):
  with p.open('rb')as f:magic=f.read(4)
  if magic==b'\x7fELF':r['executables'].append({'path':str(p.relative_to(root)),'sha256':hashlib.sha256(p.read_bytes()).hexdigest(),'bytes':p.stat().st_size})
receipt.write_text(json.dumps(r,indent=2)+'\n');assert r['clean_after']and r['steps'][0]['actual_passed']==32;print('PASS',flush=True)
