import os,subprocess,pathlib,json,hashlib,time,shutil,re
root=pathlib.Path.cwd();checkout=root/'work/6380-6377-combined-proof';target=root/'work/6380-6377-combined-target';tmp=root/'work/6380-6377-combined-tmp';tmp.mkdir(exist_ok=True);env=dict(os.environ,CARGO_TARGET_DIR=str(target),TMPDIR=str(tmp),CARGO_BUILD_JOBS='2',CARGO_INCREMENTAL='0',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0')
def git(*a):return subprocess.check_output(['git',*a],cwd=checkout,text=True).strip()
head=git('rev-parse','HEAD');assert head=='0872c37862e62d657fcd57a7740e4655008ebea1';assert git('rev-parse','HEAD^{tree}')=='05986adf4c0ad6daf2fecd83a0b0452c1d29eba7';assert not git('status','--porcelain');assert shutil.disk_usage(root).free>=7*1024**3
assert git('diff','--name-only','5b97ebbcbcbca860de344785e4a6d0a671438173','HEAD:bodies/environment/runtime').splitlines()==['tests/observation_kernel.rs','tests/support/assignment.rs']
r={'source':head,'parents':git('show','-s','--format=%P',head).split(),'tree':git('rev-parse','HEAD^{tree}'),'runtime_tree':git('rev-parse','HEAD:bodies/environment/runtime'),'environment':{k:env[k]for k in ['CARGO_TARGET_DIR','TMPDIR','CARGO_BUILD_JOBS','CARGO_INCREMENTAL','CARGO_PROFILE_DEV_DEBUG','CARGO_PROFILE_TEST_DEBUG']},'started':time.time(),'steps':[]};receipt=root/'outputs/6380-6377-combined-local-proof.json'
for capture in [False,True]:
 for operation in ['test','clippy']:
  name=('runtime-'if operation=='test'else'strict-lint-')+('capture'if capture else'default');cmd=['cargo','+stable',operation,'--locked','--manifest-path','bodies/environment/runtime/Cargo.toml'];cmd+=['--features','source-capture']if capture else [];cmd+=['--all-targets','--','-D','warnings']if operation=='clippy'else [];log=root/f'work/6380-6377-combined-{name}.log';print('START',name,cmd,flush=True)
  with log.open('wb')as f:p=subprocess.run(cmd,cwd=checkout,env=env,stdout=f,stderr=subprocess.STDOUT)
  raw=log.read_bytes();row={'name':name,'command':cmd,'log':str(log.relative_to(root)),'sha256':hashlib.sha256(raw).hexdigest(),'exit_code':p.returncode,'actual_passed':sum(map(int,re.findall(r'test result: ok\. (\d+) passed; 0 failed; 0 ignored;',raw.decode()))),'actual_named_successes':[line for line in raw.decode().splitlines()if line.startswith('test ')and line.endswith(' ... ok')]};r['steps'].append(row);receipt.write_text(json.dumps(r,indent=2)+'\n');print({k:v for k,v in row.items()if k!='actual_named_successes'},flush=True);assert p.returncode==0
  if operation=='test':assert row['actual_passed']==(36 if capture else 35)
r.update(finished=time.time(),clean_after=not git('status','--porcelain'));r['executables']=[]
for p in target.rglob('*'):
 if p.is_file()and os.access(p,os.X_OK):
  with p.open('rb')as f:magic=f.read(4)
  if magic==b'\x7fELF':r['executables'].append({'path':str(p.relative_to(root)),'sha256':hashlib.sha256(p.read_bytes()).hexdigest(),'bytes':p.stat().st_size})
receipt.write_text(json.dumps(r,indent=2)+'\n');assert r['clean_after'];print('PASS',len(r['executables']),'ELFs',flush=True)
