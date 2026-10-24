import pathlib,subprocess,os,json,time,hashlib,re
root=pathlib.Path.cwd();source=root/'work/6410-tape-branch-repair';target=root/'work/6410-branch-join-target';tmp=root/'work/6410-branch-join-tmp';tmp.mkdir(exist_ok=True)
g=lambda *a:subprocess.check_output(['git','-C',str(source),*a],text=True).strip()
assert not g('status','--porcelain');free=os.statvfs(root).f_bavail*os.statvfs(root).f_frsize;assert free>=7*1024**3
cmd=['cargo','+stable','test','--locked','-p','conduit-plot','--test','prepared_structured_tape','--','--nocapture','--test-threads=1'];env=dict(os.environ,CARGO_TARGET_DIR=str(target),TMPDIR=str(tmp),CARGO_BUILD_JOBS='2',CARGO_INCREMENTAL='0',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0');log=root/'work/6410-branch-join-first-negative.log';r={'source':g('rev-parse','HEAD'),'tree':g('rev-parse','HEAD^{tree}'),'command':cmd,'log':str(log),'before_free_bytes':free,'started':time.time()}
with log.open('wb')as f:r['exit_code']=subprocess.run(cmd,cwd=source,env=env,stdout=f,stderr=subprocess.STDOUT).returncode
b=log.read_bytes();r.update(finished=time.time(),sha256=hashlib.sha256(b).hexdigest(),clean_after=not g('status','--porcelain'),actual_running=re.findall(r'Running .*?\(([^\n]+)\)',b.decode(errors='replace')),status='expected_negative' if b'conditional_record_field_keeps_following_constant_field_on_both_branches ... FAILED'in b else 'inspect_actual_result');r['executables']=[]
for p in target.rglob('*'):
 if p.is_file():
  with p.open('rb')as f:magic=f.read(4)
  if magic==b'\x7fELF':r['executables'].append({'path':str(p),'bytes':p.stat().st_size,'sha256':hashlib.sha256(p.read_bytes()).hexdigest()})
(root/'outputs/6410-branch-join-first-negative.json').write_text(json.dumps(r,indent=2)+'\n');print(json.dumps({k:v for k,v in r.items()if k!='executables'},indent=2))
