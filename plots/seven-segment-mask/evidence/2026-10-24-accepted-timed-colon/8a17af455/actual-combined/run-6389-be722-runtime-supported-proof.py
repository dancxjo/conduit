import os,subprocess,pathlib,json,hashlib,time,shutil,re
root=pathlib.Path.cwd();checkout=pathlib.Path('/tmp/conduit-6389-be722-source');target=pathlib.Path('/tmp/conduit-6389-be722-target');tmp=pathlib.Path('/tmp/conduit-6389-be722-tmp');tmp.mkdir(exist_ok=True);env=dict(os.environ,CARGO_TARGET_DIR=str(target),TMPDIR=str(tmp),CARGO_BUILD_JOBS='2',CARGO_INCREMENTAL='0',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0')
def git(*a):return subprocess.check_output(['git',*a],cwd=checkout,text=True).strip()
head=git('rev-parse','HEAD');assert head=='f7a959cdc7304f74d6b058f69f139af94037bab5';tree=git('rev-parse','HEAD^{tree}');assert tree=='be722c7c85e0553256f1cb4c42ac6919081f3755';assert not git('status','--porcelain');r={'source':head,'tree':tree,'runtime_tree':git('rev-parse','HEAD:bodies/environment/runtime'),'mask_tree':git('rev-parse','HEAD:plots/seven-segment-mask'),'fore_tree':git('rev-parse','HEAD:architecture/assigned-plan'),'composition_verified':True,'actual_Make_8path_extension_after_frozen32':True,'environment':{k:env[k]for k in ['CARGO_TARGET_DIR','TMPDIR','CARGO_BUILD_JOBS','CARGO_INCREMENTAL','CARGO_PROFILE_DEV_DEBUG','CARGO_PROFILE_TEST_DEBUG']},'steps':[],'started':time.time()};receipt=root/'outputs/6389-be722-runtime-supported-local-proof.json';steps=[]
for capture in [False,True]:
 for op in ['test','clippy']:
  cmd=['cargo','+stable',op,'--locked','--manifest-path','bodies/environment/runtime/Cargo.toml'];cmd+=['--features','source-capture']if capture else [];cmd+=['--all-targets','--','-D','warnings']if op=='clippy'else [];steps.append(('runtime-'+('capture'if capture else'default')+('-strict'if op=='clippy'else''),cmd,(36 if capture else 35)if op=='test'else 0))
for op in ['test','clippy']:
 cmd=['cargo','+stable',op,'--locked','--manifest-path','plots/seven-segment-mask/Cargo.toml'];cmd+=['--all-targets','--','-D','warnings']if op=='clippy'else [];steps.append(('mask'+('-strict'if op=='clippy'else'-tests'),cmd,0 if op=='clippy'else 13))
for allfeatures in [False,True]:
 for op in ['test','clippy']:
  cmd=['cargo','+stable',op,'--locked','-p','conduit-assigned-plan'];cmd+=['--lib']if op=='test'else['--all-targets'];cmd+=['--all-features']if allfeatures else [];cmd+=['--','-D','warnings']if op=='clippy'else [];steps.append(('fore-'+('all-features'if allfeatures else'default')+('-strict'if op=='clippy'else'-tests'),cmd,9 if op=='test'else 0))
steps += [('supported-tm1637',['cargo','+stable','xtask','check','tm1637-output'],17),('supported-source',['cargo','+stable','xtask','prove','environment','source'],7)]
protected=[]
for file,n in [('outputs/ready-four-combined-local-proof.json',138),('outputs/ready-four-supported-supplement-local-proof.json',104),('outputs/ready-four-native-local-proof.json',81),('outputs/6389-be722-local-proof.json',103)]:
 old=json.loads((root/file).read_bytes());assert len(old['executables'])==n;protected+=old['executables']
for e in protected:assert hashlib.sha256(pathlib.Path(e['path']).read_bytes()).hexdigest()==e['sha256']
r['protected426_verified_before']=True
for name,cmd,expected in steps:
 free=shutil.disk_usage(tmp).free
 if free<7*1024**3:r.update(status='stopped-before-next-step-insufficient-headroom',next_step=name,available_bytes=free);receipt.write_text(json.dumps(r,indent=2)+'\n');raise SystemExit('Headroom below7GiB; no next command started')
 log=root/f'work/6389-be722-runtime-{name}.log';print('START',name,'free',free,cmd,flush=True)
 with log.open('wb')as f:p=subprocess.run(cmd,cwd=checkout,env=env,stdout=f,stderr=subprocess.STDOUT)
 raw=log.read_bytes();counts=list(map(int,re.findall(r'test result: ok\. (\d+) passed; 0 failed; 0 ignored;',raw.decode())));named=[line for line in raw.decode().splitlines()if line.startswith('test ')and line.endswith(' ... ok')];running=re.findall(r'Running[^\n]*\(([^)]+)\)',raw.decode());row={'name':name,'command':cmd,'log':str(log.relative_to(root)),'sha256':hashlib.sha256(raw).hexdigest(),'exit_code':p.returncode,'before_available_bytes':free,'actual_test_counts':counts,'actual_passed':sum(counts),'actual_named_successes':named,'cargo_running_executables':running};r['steps'].append(row);receipt.write_text(json.dumps(r,indent=2)+'\n');print({k:v for k,v in row.items()if k not in ['actual_named_successes','cargo_running_executables','command']},flush=True);assert p.returncode==0 and sum(counts)==expected
for e in protected:assert hashlib.sha256(pathlib.Path(e['path']).read_bytes()).hexdigest()==e['sha256']
r['protected426_verified_after']=True
r.update(finished=time.time(),clean_after=not git('status','--porcelain'),status='passed');r['executables']=[]
for p in target.rglob('*'):
 if p.is_file()and os.access(p,os.X_OK):
  with p.open('rb')as f:magic=f.read(4)
  if magic==b'\x7fELF':r['executables'].append({'path':str(p),'sha256':hashlib.sha256(p.read_bytes()).hexdigest(),'bytes':p.stat().st_size})
receipt.write_text(json.dumps(r,indent=2)+'\n');assert r['clean_after']and sum(x['actual_passed']for x in r['steps'])==126;print('PASS',len(r['executables']),'ELFs',flush=True)
