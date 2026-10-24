import pathlib,subprocess,json,os,time,hashlib,re,shutil
ROOT=pathlib.Path.cwd();W=ROOT/'work/6411-payload-ceiling-source';T=ROOT/'work/6411-payload-ceiling-target';TMP=ROOT/'work/6411-payload-ceiling-tmp';TMP.mkdir(exist_ok=True);S='cf42934dcfeadd19d645c4317e9aa71092c4865c';git=lambda *a:subprocess.check_output(['git','-C',str(W),*a],text=True).strip();assert git('rev-parse','HEAD')==S and not git('status','--porcelain');env=dict(os.environ,CARGO_TARGET_DIR=str(T),TMPDIR=str(TMP),CARGO_BUILD_JOBS='2',CARGO_INCREMENTAL='0',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0');r={'source':S,'tree':git('rev-parse','HEAD^{tree}'),'started':time.time(),'steps':[],'environment':{k:env[k]for k in ['CARGO_TARGET_DIR','TMPDIR','CARGO_BUILD_JOBS','CARGO_INCREMENTAL','CARGO_PROFILE_DEV_DEBUG','CARGO_PROFILE_TEST_DEBUG']},'source_blobs':{p:git('rev-parse',S+':'+p)for p in git('diff','--name-only',S+'^',S).splitlines()},'root_inputs':{p:git('rev-parse',S+':'+p)for p in ['Cargo.toml','Cargo.lock','rust-toolchain.toml','.cargo/config.toml']}};receipt=ROOT/'outputs/6411-payload-ceiling-local-proof.json';commands=[('supported-display-output',['cargo','+stable','xtask','prove','environment','display-output'],8)]
for name,cmd,n in commands:
 free=shutil.disk_usage(ROOT).free
 if free<7*1024**3:r.update(status='stopped-headroom-before-next-command',available_bytes=free);receipt.write_text(json.dumps(r,indent=2)+'\n');raise SystemExit('Headroomguard')
 log=ROOT/f'work/6411-{name}.log';print('START',name,free,flush=True)
 with log.open('wb')as f:p=subprocess.run(cmd,cwd=W,env=env,stdout=f,stderr=subprocess.STDOUT)
 b=log.read_bytes();counts=list(map(int,re.findall(rb'test result: ok\. (\d+) passed; 0 failed; 0 ignored;',b)));row={'name':name,'command':cmd,'exit_code':p.returncode,'before_available_bytes':free,'log':str(log),'sha256':hashlib.sha256(b).hexdigest(),'test_counts':counts,'actual_passed':sum(counts),'actual_test_Running':re.findall(r'^\s*Running .*?\(([^)]+)\)\s*$',b.decode(),re.M),'actual_command_Running':re.findall(r'^\s*Running `([^`]+)`',b.decode(),re.M)};r['steps'].append(row);r['status']='running'if p.returncode==0 and sum(counts)==n else'first-negative-preserved';receipt.write_text(json.dumps(r,indent=2)+'\n');print(name,p.returncode,counts,flush=True)
 if r['status']=='first-negative-preserved':raise SystemExit('Firstnegativepreserved')
r.update(status='passed',clean_after=not git('status','--porcelain'),finished=time.time(),executables=[])
paths=set()
for root in [T,W/'target/xtask-host-release']:
 if not root.exists():continue
 for p in root.rglob('*'):
  if p.is_file()and os.access(p,os.X_OK):
   with p.open('rb')as f:magic=f.read(4)
   if magic==b'\x7fELF':paths.add(p.resolve())
for step in r['steps']:
 for name in [*step['actual_test_Running'],*[x.split(' ',1)[0]for x in step['actual_command_Running']]]:
  p=pathlib.Path(name);p=p if p.is_absolute()else W/p;paths.add(p.resolve())
for p in sorted(paths):
 b=p.read_bytes();assert b[:4]==b'\x7fELF';r['executables'].append({'path':str(p),'bytes':len(b),'sha256':hashlib.sha256(b).hexdigest(),'in_private_target':p.is_relative_to(T)})
receipt.write_text(json.dumps(r,indent=2)+'\n');assert r['clean_after'];print('PASS',len(r['executables']),flush=True)
