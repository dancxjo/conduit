import os,subprocess,pathlib,json,time,hashlib,re
root=pathlib.Path.cwd();source=root/'work/6393-permanent-cord-bound';target=root/'work/6393-permanent-cord-target'
def git(*a):return subprocess.check_output(['git','-C',str(source),*a],text=True).strip()
head=git('rev-parse','HEAD');assert head=='9485b9e3d2a2ddec797ff4054b8c68066245d865' and not git('status','--porcelain');env=dict(os.environ,CARGO_TARGET_DIR=str(target),CARGO_BUILD_JOBS='4',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0',CARGO_INCREMENTAL='0')
steps=[('quantity-kernel',['test','--locked','-p','conduit-std-host','--lib','installed_std_tests::math_conformance','--','--nocapture']),('completion-pressure',['test','--locked','-p','conduit-std-host','--lib','installed_std::math_backs::tests','--','--nocapture']),('planner',['test','--locked','-p','conduit-planner','--lib']),('catalog',['test','--locked','-p','conduit-semantic-catalog','--all-features','quantity_']),('scoped-strict',['clippy','--locked','-p','conduit-planner','-p','conduit-semantic-catalog','-p','conduit-std-host','--lib','--','-D','warnings'])]
r={'source':head,'tree':git('rev-parse','HEAD^{tree}'),'steps':[],'status':'running'}
for name,args in steps:
 if os.statvfs(root).f_bavail*os.statvfs(root).f_frsize<5*1024**3:r['status']='stopped_before_next_step_for_headroom';break
 log=root/('work/6393-permanent-cord-'+name+'.log');argv=['cargo','+stable',*args];start=time.time()
 with log.open('wb')as out:p=subprocess.run(argv,cwd=source,env=env,stdout=out,stderr=subprocess.STDOUT)
 b=log.read_bytes();s=b.decode(errors='replace');elfs=[]
 for path in re.findall(r'Running[^\n]*\(([^\n)]+)\)',s):
  path=pathlib.Path(path);path=path if path.is_absolute()else source/path;data=path.read_bytes()
  if data[:4]==b'\x7fELF':elfs.append({'path':str(path),'bytes':len(data),'sha256':hashlib.sha256(data).hexdigest()})
 r['steps'].append({'name':name,'argv':argv,'start':start,'end':time.time(),'exit_code':p.returncode,'log':str(log),'log_sha256':hashlib.sha256(b).hexdigest(),'actual_test_executables':elfs,'test_summaries':re.findall(r'test result:[^\n]+',s)});print(name,p.returncode,flush=True)
 if p.returncode:r['status']='failed';break
else:r['status']='passed'
r['clean_after']=not git('status','--porcelain');assert git('rev-parse','HEAD')==head
(root/'outputs/6393-permanent-cord-positive.json').write_text(json.dumps(r,indent=2));print(json.dumps(r,indent=2))
