import pathlib,subprocess,os,json,time,hashlib,re
root=pathlib.Path.cwd();source=root/'work/6410-tape-branch-repair';target=root/'work/6410-branch-join-target';tmp=root/'work/6410-branch-join-tmp';g=lambda *a:subprocess.check_output(['git','-C',str(source),*a],text=True).strip();assert not g('status','--porcelain');env=dict(os.environ,CARGO_TARGET_DIR=str(target),TMPDIR=str(tmp),CARGO_BUILD_JOBS='2',CARGO_INCREMENTAL='0',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0');targets=['bme280_lifecycle','device_frames'];args=[x for t in targets for x in ['--test',t]];r={'source':g('rev-parse','HEAD'),'tree':g('rev-parse','HEAD^{tree}'),'started':time.time(),'steps':[],'environment':{k:env[k]for k in ['CARGO_TARGET_DIR','TMPDIR','CARGO_BUILD_JOBS','CARGO_INCREMENTAL','CARGO_PROFILE_DEV_DEBUG','CARGO_PROFILE_TEST_DEBUG']}}
for name,cmd in [('owning-regressions',['cargo','+stable','test','--locked','-p','conduit-plot',*args,'--','--nocapture','--test-threads=1']),('scoped-strict',['cargo','+stable','clippy','--locked','-p','conduit-plot','--lib',*args,'--','-D','warnings'])]:
 free=os.statvfs(root).f_bavail*os.statvfs(root).f_frsize;assert free>=7*1024**3
 log=root/('work/6410-ci-failure-supplement-'+name+'.log')
 with log.open('wb')as f:code=subprocess.run(cmd,cwd=source,env=env,stdout=f,stderr=subprocess.STDOUT).returncode
 b=log.read_bytes();row={'name':name,'command':cmd,'exit_code':code,'log':str(log),'sha256':hashlib.sha256(b).hexdigest(),'actual_passed_counts':[int(x)for x in re.findall(rb'test result: ok\. (\d+) passed',b)],'actual_running':re.findall(r'Running .*?\(([^\n]+)\)',b.decode(errors='replace')),'before_free_bytes':free};r['steps'].append(row)
 if code:break
r.update(finished=time.time(),clean_after=not g('status','--porcelain'),status='passed' if len(r['steps'])==2 and all(s['exit_code']==0 for s in r['steps'])else'failed');r['executables']=[]
for p in target.rglob('*'):
 if p.is_file():
  with p.open('rb')as f:magic=f.read(4)
  if magic==b'\x7fELF':r['executables'].append({'path':str(p),'bytes':p.stat().st_size,'sha256':hashlib.sha256(p.read_bytes()).hexdigest()})
(root/'outputs/6410-ci-failure-supplement-positive.json').write_text(json.dumps(r,indent=2)+'\n');print(json.dumps({k:v for k,v in r.items()if k!='executables'},indent=2))
